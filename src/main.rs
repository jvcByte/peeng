mod api;
mod shared;

use crate::api::home::routes::home_routes;
use crate::api::refresh_tokens::repository::RefreshTokenRepository;
use crate::api::routes::routes;
use crate::shared::config::load_env_var::{EnvVariables, JwtConfig};
use crate::shared::config::{app_state::AppState, postgres};
use actix_cors::Cors;
use actix_web::http::header;
use actix_web::middleware::NormalizePath;
use actix_web::{App, HttpServer, middleware::Logger, web};
use dotenvy::dotenv;
use env_logger::Env;
use log::{error, info};
use migration::{Migrator, MigratorTrait};
use tokio::time::{Duration, interval};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    let env = Env::default().filter_or("RUST_LOG", "debug");
    env_logger::Builder::from_env(env).init();

    JwtConfig::init();
    EnvVariables::init();

    let db = match postgres::init_db().await {
        Ok(db) => db,
        Err(e) => {
            error!("failed to initialize database: {}", e);
            std::process::exit(1);
        }
    };
    if let Err(e) = Migrator::up(&db, None).await {
        error!("failed to run migrations: {}", e);
        std::process::exit(1);
    }

    // Spawn background task to clean up expired refresh tokens every 6 hours
    let cleanup_db = db.clone();
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(6 * 60 * 60));
        loop {
            ticker.tick().await;
            match RefreshTokenRepository::delete_expired(&cleanup_db).await {
                Ok(n) => info!("cleanup: deleted {} expired refresh tokens", n),
                Err(e) => error!("cleanup: failed to delete expired tokens: {}", e),
            }
        }
    });

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
