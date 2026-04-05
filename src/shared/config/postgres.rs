use crate::shared::config::load_env_var::EnvVariables;
use crate::shared::utils::config_utils::redact_url_password;
use log::info;
use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};
use std::error::Error;
use std::time::Duration;

pub async fn init_db() -> Result<DatabaseConnection, Box<dyn Error + Send + Sync>> {
    let database_url = EnvVariables::get().db_url.clone();

    info!(
        "Connecting to database: {}",
        redact_url_password(&database_url)
    );

    let mut opt = ConnectOptions::new(database_url);
    opt.max_connections(100)
        .min_connections(5)
        .connect_timeout(Duration::from_secs(8))
        .acquire_timeout(Duration::from_secs(8))
        .idle_timeout(Duration::from_secs(8))
        .max_lifetime(Duration::from_secs(8))
        .sqlx_logging(true)
        .sqlx_logging_level(log::LevelFilter::Debug);

    let db = Database::connect(opt)
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

    info!("✅ Database connected successfully ✅");
    Ok(db)
}

pub async fn check_connection(db: &DatabaseConnection) -> Result<(), DbErr> {
    db.ping().await
}
