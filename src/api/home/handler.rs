use crate::shared::config::app_state::AppState;
use crate::shared::config::postgres::check_connection;
use actix_web::{HttpResponse, Responder, web};

pub async fn app_details() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({"Info" : "App Details Coming Soon!"}))
}

/// Simple health-check endpoint so you can verify the server is running.
pub async fn health() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({ "status": "ok" }))
}

pub async fn check_db_connection(state: web::Data<AppState>) -> impl Responder {
    match check_connection(&state.db).await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({
            "status": "Database connection successful"
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("Database connection failed: {}", e)
        })),
    }
}
