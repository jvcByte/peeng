use crate::shared::models::refresh_tokens::{RefreshToken, refresh_token};
use crate::shared::utils::auth_utils::hash_token;
use chrono::Utc;
use sea_orm::prelude::DateTimeWithTimeZone;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, Set,
};
use uuid::Uuid;

pub struct RefreshTokenRepository;

impl RefreshTokenRepository {
    /// Persist a new refresh token record.
    /// `token` must be the plaintext token — this method hashes it before storing.
    pub async fn create(
        db: &impl ConnectionTrait,
        user_id: Uuid,
        token: String,
        expires_at: Option<DateTimeWithTimeZone>,
    ) -> Result<refresh_token::Model, DbErr> {
        let token_hash = hash_token(&token);
        let active = refresh_token::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            token: Set(token_hash),
            revoked: Set(false),
            expires_at: Set(expires_at),
            created_at: Set(Some(Utc::now().into())),
        };
        RefreshToken::insert(active).exec_with_returning(db).await
    }

    /// Find a single active (non-revoked, non-expired) token by hashing the presented plaintext
    /// and looking up the hash via the DB index.
    pub async fn find_active_by_token(
        db: &DatabaseConnection,
        token: &str,
    ) -> Result<Option<refresh_token::Model>, DbErr> {
        let token_hash = hash_token(token);
        let now: DateTimeWithTimeZone = Utc::now().into();
        RefreshToken::find()
            .filter(refresh_token::Column::Token.eq(token_hash))
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
    pub async fn revoke_by_id(db: &impl ConnectionTrait, id: Uuid) -> Result<(), DbErr> {
        let rows = RefreshToken::update_many()
            .col_expr(
                refresh_token::Column::Revoked,
                sea_orm::sea_query::Expr::value(true),
            )
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
            .col_expr(
                refresh_token::Column::Revoked,
                sea_orm::sea_query::Expr::value(true),
            )
            .filter(refresh_token::Column::UserId.eq(user_id))
            .filter(refresh_token::Column::Revoked.eq(false))
            .exec(db)
            .await
            .map(|res| res.rows_affected)
    }

    /// Delete all expired refresh tokens — single DB-level DELETE.
    pub async fn delete_expired(db: &DatabaseConnection) -> Result<u64, DbErr> {
        let now: DateTimeWithTimeZone = Utc::now().into();
        RefreshToken::delete_many()
            .filter(refresh_token::Column::ExpiresAt.lt(now))
            .exec(db)
            .await
            .map(|res| res.rows_affected)
    }
}
