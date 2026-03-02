use actix_web::{App, HttpResponse, HttpServer, Responder, middleware::Logger, web};
use dotenvy::dotenv;
use env_logger::Env;
use log::info;

async fn health() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({"status": "Ok"}))
}
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    let env = Env::default().filter_or("RUST_LOG", "info");
    env_logger::Builder::from_env(env).init();
    info!("Starting server at http://127.0.0.1:8080");

    HttpServer::new(|| {
        App::new()
            .wrap(Logger::default())
            .route("/health", web::get().to(health))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
