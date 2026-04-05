mod api;
mod shared;

use crate::api::home::routes::home_routes;
use crate::api::routes::routes;

use crate::shared::config::load_env_var::{EnvVariables, JwtConfig};
use crate::shared::config::{app_state::AppState, postgres};
use actix_cors::Cors;
use actix_web::http::header;
use actix_web::middleware::NormalizePath;
use actix_web::{App, HttpServer, middleware::Logger, web};
use dotenvy::dotenv;
use env_logger::Env;
use log::error;
use migration::{Migrator, MigratorTrait};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load .env (if present) and initialize logging.
    dotenv().ok();
    let env = Env::default().filter_or("RUST_LOG", "info");
    env_logger::Builder::from_env(env).init();

    // Validate and cache all config from environment at startup.
    // This panics immediately if required vars (e.g. JWT_SECRET) are missing,
    // rather than surfacing as a 500 error on the first authenticated request.
    JwtConfig::init();
    EnvVariables::init();

    // Initialize DB connection via the postgres module. This requires the
    // `DATABASE_URL` environment variable to be set. No secrets are hardcoded here.
    let db = match postgres::init_db().await {
        Ok(db) => db,
        Err(e) => {
            error!("failed to initialize database: {}", e);
            // Exit with non-zero status so orchestrators/CI notice startup failure.
            std::process::exit(1);
        }
    };
    if let Err(e) = Migrator::up(&db, None).await {
        error!("failed to run migrations: {}", e);
        std::process::exit(1);
    }

    // Build application state and start server.
    let state = web::Data::new(AppState::new(db));

    let address = EnvVariables::get().address.clone();
    let port = EnvVariables::get().port.clone();
    let base_url = format!("{}:{}", address, port);

    HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin("http://localhost:1420")
            .allowed_origin("http://localhost:3000")
            .allowed_origin("https://tauri.localhost")
            .allowed_origin("http://tauri.localhost")
            .allowed_origin("tauri://localhost")
            .allowed_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"])
            .allowed_headers(vec![
                header::AUTHORIZATION,
                header::CONTENT_TYPE,
                header::ACCEPT,
            ])
            .expose_headers(vec![header::CONTENT_TYPE])
            .max_age(3600)
            .supports_credentials();

        App::new()
            .wrap(Logger::default())
            .wrap(NormalizePath::trim())
            .wrap(cors)
            .app_data(state.clone())
            .configure(home_routes)
            .configure(routes)
    })
    .bind(base_url)?
    .run()
    .await
}
