use crate::api::refresh_tokens::repository::RefreshTokenRepository;
use crate::api::users::repository::UserRepository;
use actix_web::{Error, FromRequest, HttpRequest, dev::Payload, error, http::header, web};
use futures::future::LocalBoxFuture;
use uuid::Uuid;

use crate::shared::config::app_state::AppState;
use crate::shared::config::load_env_var::JwtConfig;
use crate::shared::utils::auth_utils::decode_jwt;

/// Authenticated user extracted from a valid Bearer JWT on each request.
#[derive(Clone, Debug)]
pub struct AuthenticatedUser {
    pub id: Uuid,
    pub name: String,
    pub email: String,
}

/// Actix extractor: add `user: AuthenticatedUser` to any handler to require authentication.
///
/// Steps:
/// 1. Parse `Authorization: Bearer <token>` header (case-insensitive scheme).
/// 2. Decode and validate the JWT (signature + expiry).
/// 3. Look up the user in DB — reject if not found or inactive.
/// 4. If the token carries a `tv` claim, verify it matches the stored `token_version`.
/// 5. Confirm the user has at least one active (non-revoked, non-expired) refresh token.
///    Steps 4 and 5 share a single DB query.
impl FromRequest for AuthenticatedUser {
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let auth_header = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_owned());

        let app_data = req.app_data::<web::Data<AppState>>().cloned();

        Box::pin(async move {
            // 1) Ensure Authorization header is present
            let auth = auth_header
                .ok_or_else(|| error::ErrorUnauthorized("Missing Authorization header"))?;

            // 2) Parse "Bearer <token>" — case-insensitive per HTTP spec
            let token = auth
                .split_once(' ')
                .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
                .map(|(_, t)| t.trim())
                .filter(|t| !t.is_empty())
                .ok_or_else(|| error::ErrorUnauthorized("Invalid Authorization header"))?;

            // 3) Decode and validate JWT (signature + expiry checked by jsonwebtoken)
            let cfg = JwtConfig::get();
            let token_data = decode_jwt(token, &cfg)
                .map_err(|_| error::ErrorUnauthorized("Invalid or expired token"))?;

            // 4) Parse subject as UUID
            let user_id = Uuid::parse_str(&token_data.claims.sub)
                .map_err(|_| error::ErrorUnauthorized("Invalid token subject"))?;

            // 5) Resolve AppState
            let state = app_data
                .ok_or_else(|| error::ErrorInternalServerError("Missing app state"))?;

            let db = &state.db;

            // 6) Look up user — reject if not found or account is disabled
            let user = UserRepository::find_by_id(db, user_id)
                .await
                .map_err(|_| error::ErrorInternalServerError("Failed to look up user"))?
                .ok_or_else(|| error::ErrorUnauthorized("User not found"))?;

            if !user.is_active {
                return Err(error::ErrorUnauthorized("Account is disabled"));
            }

            // 7) Single query: fetch the current active refresh token for this user.
            //    Ordered by created_at DESC so rotation always returns the latest record.
            let tk = RefreshTokenRepository::find_active_by_user_id(db, user_id)
                .await
                .map_err(|_| error::ErrorInternalServerError("Failed to look up session"))?
                .ok_or_else(|| error::ErrorUnauthorized("No active session"))?;

            // 8) If the JWT carries a tv claim, verify it matches the stored version.
            //    Tokens without tv (e.g. legacy) skip this check.
            if let Some(token_tv) = token_data.claims.tv {
                if token_tv != tk.token_version {
                    return Err(error::ErrorUnauthorized("Token has been revoked"));
                }
            }

            Ok(AuthenticatedUser {
                id: user.id,
                name: user.name,
                email: user.email,
            })
        })
    }
}
