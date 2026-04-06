use actix_web::web;
use actix_governor::{Governor, GovernorConfigBuilder};

use crate::api::auth::handlers::{
    login, logout, logout_all, me, refresh, register,
};

/// Mount authentication routes under `/auth`.
///
/// Routes:
/// - POST /auth/register   -> register a new user (returns tokens + user info)
/// - POST /auth/login      -> login with credentials (returns tokens + user info)
/// - POST /auth/refresh    -> refresh access token using a refresh token
/// - POST /auth/logout     -> revoke refresh token / logout
/// - POST /auth/logout-all -> revoke all refresh tokens for user (global logout)
/// - GET  /auth/me         -> get current authenticated user (requires Authorization header)
pub fn auth_routes(cfg: &mut web::ServiceConfig) {
    // 2 requests per second per IP on sensitive auth endpoints, burst of 5
    let governor_conf = GovernorConfigBuilder::default()
        .requests_per_second(2)
        .burst_size(5)
        .finish()
        .expect("Invalid rate limit config");

    cfg.service(
        web::scope("/auth")
            .wrap(Governor::new(&governor_conf))
            .route("/register", web::post().to(register))
            .route("/login", web::post().to(login))
            .route("/refresh", web::post().to(refresh))
            .route("/logout", web::post().to(logout))
            .route("/logout-all", web::post().to(logout_all))
            .route("/me", web::get().to(me)),
    );
}
