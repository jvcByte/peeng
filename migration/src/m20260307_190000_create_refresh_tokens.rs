use entity::refresh_tokens::{RefreshToken, refresh_token};
use entity::users::{User, user};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create refresh_tokens table
        manager
            .create_table(
                Table::create()
                    .table(RefreshToken)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(refresh_token::Column::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(refresh_token::Column::UserId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(refresh_token::Column::Token)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(refresh_token::Column::TokenVersion)
                            .integer()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(refresh_token::Column::Revoked)
                            .boolean()
                            .not_null()
                            .default(Value::Bool(Some(false))),
                    )
                    .col(
                        ColumnDef::new(refresh_token::Column::ExpiresAt).timestamp_with_time_zone(),
                    )
                    .col(
                        ColumnDef::new(refresh_token::Column::CreatedAt).timestamp_with_time_zone(),
                    )
                    // Foreign key to users(id)
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_refresh_tokens_user")
                            .from(RefreshToken, refresh_token::Column::UserId)
                            .to(User, user::Column::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Index on token_hash for quick lookup (used when validating presented refresh tokens)
        manager
            .create_index(
                Index::create()
                    .name("idx_refresh_tokens_token_hash")
                    .table(RefreshToken)
                    .col(refresh_token::Column::Token)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop index then table
        manager
            .drop_index(
                Index::drop()
                    .name("idx_refresh_tokens_token_hash")
                    .table(RefreshToken)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(RefreshToken).to_owned())
            .await
    }
}
