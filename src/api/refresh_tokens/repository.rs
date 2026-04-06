use crate::shared::models::refresh_tokens::{RefreshToken, refresh_token};
use chrono::Utc;
use sea_orm::prelude::DateTimeWithTimeZone;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, Set,
};
use uuid::Uuid;

pub struct RefreshTokenRepository;

impl RefreshTokenRepository {
    /// Create and persist a new refresh token record.
    pub async fn create(
        db: &DatabaseConnection,
        user_id: Uuid,
        token: String,
        expires_at: Option<DateTimeWithTimeZone>,
    ) -> Result<refresh_token::Model, DbErr> {
        let active = refresh_token::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            token: Set(token),
            token_version: Set(0),
            revoked: Set(false),
            expires_at: Set(expires_at),
            created_at: Set(Some(Utc::now().into())),
        };
        RefreshToken::insert(active).exec_with_returning(db).await
    }

    pub async fn update(
        db: &DatabaseConnection,
        model: refresh_token::ActiveModel,
    ) -> Result<refresh_token::Model, DbErr> {
        model.update(db).await
    }

    /// Find the first refresh token record for a user (any revocation state).
    /// Used to read `token_version` during auth.
    pub async fn find_by_user_id(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> Result<Option<refresh_token::Model>, DbErr> {
        RefreshToken::find()
            .filter(refresh_token::Column::UserId.eq(user_id))
            .one(db)
            .await
    }

    /// Find a single active (non-revoked, non-expired) token by its plaintext value.
    /// Uses the DB index on `token` — no full table scan.
    pub async fn find_active_by_token(
        db: &DatabaseConnection,
        token: &str,
    ) -> Result<Option<refresh_token::Model>, DbErr> {
        let now: DateTimeWithTimeZone = Utc::now().into();
        RefreshToken::find()
            .filter(refresh_token::Column::Token.eq(token))
            .filter(refresh_token::Column::Revoked.eq(false))
            .filter(
                sea_orm::Condition::any()
                    .add(refresh_token::Column::ExpiresAt.is_null())
                    .add(refresh_token::Column::ExpiresAt.gt(now)),
            )
            .one(db)
            .await
    }

    /// Revoke a single token by id — single UPDATE, no prior SELECT.
    pub async fn revoke_by_id(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<(), DbErr> {
        let rows = RefreshToken::update_many()
            .col_expr(refresh_token::Column::Revoked, sea_orm::sea_query::Expr::value(true))
            .filter(refresh_token::Column::Id.eq(id))
            .exec(db)
            .await?
            .rows_affected;

        if rows == 0 {
            Err(DbErr::RecordNotFound(format!("refresh token {} not found", id)))
        } else {
            Ok(())
        }
    }

    /// Revoke all active refresh tokens for a user — single UPDATE query.
    pub async fn revoke_by_user(db: &DatabaseConnection, user_id: Uuid) -> Result<u64, DbErr> {
        RefreshToken::update_many()
            .col_expr(refresh_token::Column::Revoked, sea_orm::sea_query::Expr::value(true))
            .filter(refresh_token::Column::UserId.eq(user_id))
            .filter(refresh_token::Column::Revoked.eq(false))
            .exec(db)
            .await
            .map(|res| res.rows_affected)
    }

    /// Delete all expired refresh tokens — single DB-level DELETE, no Rust-side filtering.
    pub async fn delete_expired(db: &DatabaseConnection) -> Result<u64, DbErr> {
        let now: DateTimeWithTimeZone = Utc::now().into();
        RefreshToken::delete_many()
            .filter(refresh_token::Column::ExpiresAt.lt(now))
            .exec(db)
            .await
            .map(|res| res.rows_affected)
    }
}
