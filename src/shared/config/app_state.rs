use sea_orm::DatabaseConnection;

/// Shared application state available to Actix handlers.
///
/// Handlers typically receive this as `web::Data<AppState>`:
/// ```ignore
/// async fn handler(state: web::Data<AppState>) { /* ... */ }
/// ```
#[derive(Clone)]
pub struct AppState {
    /// SeaORM database connection (internally reference-counted and cheap to clone).
    pub db: DatabaseConnection,
}

impl AppState {
    /// Create a new `AppState` from a `DatabaseConnection`.
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}
