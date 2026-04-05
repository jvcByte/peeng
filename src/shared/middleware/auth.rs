use crate::api::auth::repository::RefreshTokenRepository;
use crate::shared::models::refresh_tokens::{RefreshToken, refresh_token};
use actix_web::{Error, FromRequest, HttpRequest, dev::Payload, error, http::header, web};
use futures::future::LocalBoxFuture;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::shared::config::app_state::AppState;
use crate::shared::config::load_env_var::JwtConfig;
use crate::shared::utils::auth_utils::decode_jwt;

/// Authenticated user extracted by the middleware / extractor.
/// This contains only the fields commonly needed by handlers. You can expand it as needed.
#[derive(Clone, Debug)]
pub struct AuthenticatedUser {
    pub id: Uuid,
    pub name: String,
    pub email: String,
}

impl AuthenticatedUser {
    /// Helper to build an unauthorized actix_web::Error from an ApiError variant.
    fn err_unauthorized<E: Into<String>>(message: E) -> Error {
        // Respond with a 401 and a simple message. The project defines ApiError,
        // but Actix handlers expect actix_web::Error. We return a generic Unauthorized.
        error::ErrorUnauthorized(message.into())
    }
}

/// Actix extractor: `AuthenticatedUser` can be used as a function parameter to require
/// authentication on a handler. Example:
/// async fn my_handler(user: AuthenticatedUser) { ... }
///
/// Behavior:
/// - Looks for `Authorization: Bearer <token>` header.
/// - Reads `JWT_SECRET` from environment (required).
/// - Decodes & validates JWT (exp checked).
/// - Looks up the user in DB by the `sub` claim (UUID).
/// - If the user exists, returns `AuthenticatedUser`. Otherwise 401.
///
/// Note: For production revocation/security you may:
/// - Add a `token_version` column to users and verify it against a `tv` claim.
/// - Use rotating secrets / key IDs (kid) and verify via a JWKS endpoint for third-party tokens.
/// - Use refresh tokens with a stored revocation list.
impl FromRequest for AuthenticatedUser {
    type Error = Error;
    // We need an async future because DB access is required.
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        // Clone minimal things needed into the async block.
        let auth_header = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_owned());

        // Retrieve AppState (to access DB). If the handler didn't register AppState, this fails.
        // The main app constructs AppState and registers it as Data<AppState>.
        let app_data = req.app_data::<web::Data<AppState>>().cloned();

        Box::pin(async move {
            // 1) Ensure Authorization header present
            let auth = match auth_header {
                Some(a) => a,
                None => {
                    return Err(AuthenticatedUser::err_unauthorized(
                        "Missing Authorization header",
                    ));
                }
            };

            // 2) Expect "Bearer <token>"
            let token = if let Some(stripped) = auth.strip_prefix("Bearer ") {
                stripped.trim()
            } else {
                return Err(AuthenticatedUser::err_unauthorized(
                    "Invalid Authorization scheme",
                ));
            };

            if token.is_empty() {
                return Err(AuthenticatedUser::err_unauthorized("Empty bearer token"));
            }

            // 3) Decode & validate token using the shared helper (centralizes JWT config & validation)
            let cfg = JwtConfig::get();
            let token_data = match decode_jwt(token, &cfg) {
                Ok(td) => td,
                Err(_) => {
                    return Err(AuthenticatedUser::err_unauthorized(
                        "Invalid or expired token",
                    ));
                }
            };

            // 5) Parse subject as UUID
            let user_id = match Uuid::parse_str(&token_data.claims.sub) {
                Ok(u) => u,
                Err(_) => return Err(AuthenticatedUser::err_unauthorized("Invalid token subject")),
            };

            // 6) Ensure AppState/DB is available
            let state = match app_data {
                Some(d) => d,
                None => {
                    return Err(AuthenticatedUser::err_unauthorized(
                        "Server misconfiguration: missing app state",
                    ));
                }
            };

            // 7) Lookup user in DB
            // We query the entity defined in the `entity` crate. The repository pattern elsewhere wraps these calls;
            // here we query directly for simplicity. This requires the `entity` crate to be present in the workspace.
            let db = &state.db;
            let user = match entity::users::User::find_by_id(user_id).one(db).await {
                Ok(opt) => opt,
                Err(err) => {
                    return Err(AuthenticatedUser::err_unauthorized(err.to_string()));
                }
            };

            let user = match user {
                Some(u) => u,
                None => return Err(AuthenticatedUser::err_unauthorized("User not found")),
            };

            // 8) Lookup token_version in DB
            // We query the entity defined in the `entity` crate. The repository pattern elsewhere wraps these calls;
            // here we query directly for simplicity. This requires the `entity` crate to be present in the workspace.
            let refresh_token = match RefreshToken::find()
                .filter(refresh_token::Column::UserId.eq(user_id.to_owned()))
                .one(db)
                .await
            {
                Ok(opt) => opt,
                Err(err) => {
                    return Err(AuthenticatedUser::err_unauthorized(err.to_string()));
                }
            };

            let tk = match refresh_token {
                Some(t) => t,
                None => {
                    return Err(AuthenticatedUser::err_unauthorized(
                        "Refresh Token not found",
                    ));
                }
            };

            // 8) Token version check for revocation:
            // Verify the token's `tv` claim (if present) matches the user's `token_version`.
            //
            // This allows immediate revocation of access tokens by incrementing the user's
            // `token_version` in the database. The `exp` claim is also read here to silence
            // dead-code warnings (jsonwebtoken already validates expiry during decode).
            if let Some(token_tv) = Some(token_data.claims.tv) {
                // `user.token_version` is stored on the user model (i32).
                if token_tv != tk.token_version {
                    return Err(AuthenticatedUser::err_unauthorized(
                        "Token has been revoked",
                    ));
                }
            }
            // Touch `exp` so the field is considered used by the compiler (it is still validated above).
            let _ = token_data.claims.exp;

            // 8b) Ensure the user still has an active (non-revoked) refresh token record.
            // Use an efficient DB-level check to avoid loading all tokens into memory.
            let has_active_for_user =
                match RefreshTokenRepository::has_active_for_user(db, user_id).await {
                    Ok(b) => b,
                    Err(_) => {
                        return Err(AuthenticatedUser::err_unauthorized(
                            "Failed to verify session state",
                        ));
                    }
                };
            if !has_active_for_user {
                return Err(AuthenticatedUser::err_unauthorized(
                    "Token revoked or no active session",
                ));
            }

            // 9) Build AuthenticatedUser
            let out = AuthenticatedUser {
                id: user.id,
                name: user.name,
                email: user.email,
            };

            Ok(out)
        })
    }
}
