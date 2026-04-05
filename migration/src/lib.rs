pub use sea_orm_migration::prelude::*;

mod m20260307_175048_create_users;
mod m20260307_190000_create_refresh_tokens;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260307_175048_create_users::Migration),
            Box::new(m20260307_190000_create_refresh_tokens::Migration),
        ]
    }
}
