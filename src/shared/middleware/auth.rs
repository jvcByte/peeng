use crate::api::users::repository::UserRepository;
use actix_web::{Error, FromRequest, HttpRequest, dev::Payload, error, http::header, web};
use std::future::Future;
use std::pin::Pin;
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
/// 4. If the token carries a `tv` claim, verify it matches `users.token_version`.
impl FromRequest for AuthenticatedUser {
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>> + 'static>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let auth_header = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_owned());

        let app_data = req.app_data::<web::Data<AppState>>().cloned();

        Box::pin(async move {
            // 1) Ensure Authorization header is present
            let auth = auth_header.ok_or_else(|| {
                error::ErrorUnauthorized("Missing Authorization header")
            })?;

            // 2) Parse "Bearer <token>" — case-insensitive per HTTP spec
            let token = auth
                .split_once(' ')
                .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
                .map(|(_, t)| t.trim())
                .filter(|t| !t.is_empty())
                .ok_or_else(|| error::ErrorUnauthorized("Invalid Authorization header"))?;

            // 3) Decode and validate JWT (signature + expiry checked by jsonwebtoken)
            let cfg = JwtConfig::get();
            let token_data = decode_jwt(token, cfg)
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

            // 7) Verify token version against users.token_version — single source of truth.
            //    Incrementing users.token_version immediately invalidates all sessions.
            //    Tokens without a tv claim (legacy) skip this check.
            if let Some(token_tv) = token_data.claims.tv {
                if token_tv != user.token_version {
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
