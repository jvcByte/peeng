use actix_web::{App, HttpResponse, HttpServer, Responder, get, middleware::Logger, post, web};
use dotenvy::dotenv;
use env_logger::Env;
use log::info;

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello, world!")
}
#[post("/echo")]
async fn echo(req_body: String) -> impl Responder {
    HttpResponse::Ok().body(req_body)
}
async fn manual_hello() -> impl Responder {
    HttpResponse::Ok().body("Hello, There!")
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
            .service(hello)
            .service(echo)
            .route("/manual_hello", web::get().to(manual_hello))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
