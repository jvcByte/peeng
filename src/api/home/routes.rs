use crate::api::home::handler::{app_details, check_db_connection, health};
use actix_web::web;

pub fn home_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("", web::get().to(app_details));
    cfg.route("/", web::get().to(app_details));
    cfg.route("/health", web::get().to(health));
    cfg.route("/db_conn", web::get().to(check_db_connection));
}
