use actix_web::web;

use crate::api::auth::handlers::{
    cleanup_expired_tokens, login, logout, logout_all, me, refresh, register,
};

/// Mount authentication routes under `/auth`.
///
/// Routes:
/// - POST /auth/register       -> register a new user (returns tokens + user info)
/// - POST /auth/login          -> login with credentials (returns tokens + user info)
/// - POST /auth/refresh        -> refresh access token using a refresh token
/// - POST /auth/logout         -> revoke refresh token / logout
/// - POST /auth/logout-all     -> revoke all refresh tokens for user (global logout)
/// - GET  /auth/me             -> get current authenticated user (requires Authorization header)
/// - POST /auth/cleanup-tokens -> admin endpoint to clean up expired tokens
pub fn auth_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/auth")
            .route("/register", web::post().to(register))
            .route("/login", web::post().to(login))
            .route("/refresh", web::post().to(refresh))
            .route("/logout", web::post().to(logout))
            .route("/logout-all", web::post().to(logout_all))
            .route("/me", web::get().to(me))
            .route("/cleanup-tokens", web::post().to(cleanup_expired_tokens)),
    );
}
