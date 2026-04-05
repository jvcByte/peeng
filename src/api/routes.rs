//! API root module — wires feature routes under a common `/api` prefix.
//!
//! This module exposes a single `routes` function that your `main` can pass to
//! `App::configure(...)`. It delegates to feature modules (e.g. `users`) which
//! should each provide their own `pub fn routes(cfg: &mut web::ServiceConfig)`.

use actix_web::{Responder, web};

use crate::api::auth::routes::auth_routes;
use crate::api::users::routes::user_routes;

async fn available_routes() -> impl Responder {
    web::Json(serde_json::json!({
        "available_routes": [
            "/api (GET)",
            "/api/db_conn (GET)",
            "/api/health (GET)",
            "/api/users (GET)",
            "/api/users/{id} (GET, PUT, DELETE)",
            "/api/auth/register (POST)",
            "/api/auth/login (POST)",
            "/api/auth/refresh (POST)",
            "/api/auth/logout (POST)",
            "/api/auth/logout-all (POST)",
            "/api/auth/me (GET)",
            "/api/auth/cleanup-tokens (POST)"
        ]
    }))
}

/// Mount all API routes under `/api`.
///
/// Example usage from `main.rs`:
///     .configure(crud_with_sea_orm::api::routes)
pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .route("", web::get().to(available_routes))
            .route("/", web::get().to(available_routes))
            .configure(user_routes)
            .configure(auth_routes),
    );
}
