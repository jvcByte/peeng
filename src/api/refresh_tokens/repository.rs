use crate::shared::models::refresh_tokens::{RefreshToken, refresh_token};
use chrono::Utc;
use sea_orm::prelude::DateTimeWithTimeZone;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, Set,
};
use uuid::Uuid;

/// Repository for refresh token operations.
///
/// Responsibilities:
/// - create a refresh token record (store only the hashed token)
/// - lookup a refresh token by its hash
/// - revoke a single token (mark revoked)
/// - revoke all tokens for a user
/// - delete expired tokens (simple cleanup)
pub struct RefreshTokenRepository;

impl RefreshTokenRepository {
    /// Create and persist a new refresh token record.
    /// `token_hash` must already be the hashed representation of the opaque token given to the client.
    pub async fn create(
        db: &DatabaseConnection,
        user_id: Uuid,
        token: String,
        expires_at: Option<DateTimeWithTimeZone>,
    ) -> Result<refresh_token::Model, DbErr> {
        let id = Uuid::new_v4();
        let active = refresh_token::ActiveModel {
            id: Set(id),
            user_id: Set(user_id),
            token: Set(token),
            token_version: Set(0),
            revoked: Set(false),
            expires_at: Set(expires_at),
            created_at: Set(Some(Utc::now().into())),
        };

        let inserted = RefreshToken::insert(active).exec_with_returning(db).await?;
        Ok(inserted)
    }

    pub async fn update(
        db: &DatabaseConnection,
        model: refresh_token::ActiveModel,
    ) -> Result<refresh_token::Model, sea_orm::DbErr> {
        let updated = model.update(db).await?;
        Ok(updated)
    }

    pub async fn find_by_user_id(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> Result<Option<refresh_token::Model>, DbErr> {
        RefreshToken::find()
            .filter(refresh_token::Column::UserId.eq(user_id.to_owned()))
            .one(db)
            .await
    }

    /// Efficient check for whether the given user has any active (non-revoked) refresh tokens.
    /// Returns `Ok(true)` if at least one active token exists, `Ok(false)` if none, or an error.
    pub async fn has_active_for_user(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> Result<bool, DbErr> {
        let opt = RefreshToken::find()
            .filter(refresh_token::Column::UserId.eq(user_id.to_owned()))
            .filter(refresh_token::Column::Revoked.eq(false))
            .one(db)
            .await?;
        Ok(opt.is_some())
    }

    /// Find all active (non-revoked) refresh tokens.
    /// Used for token verification since Argon2 hashes can't be looked up directly.
    pub async fn find_all_active(
        db: &DatabaseConnection,
    ) -> Result<Vec<refresh_token::Model>, DbErr> {
        RefreshToken::find()
            .filter(refresh_token::Column::Revoked.eq(false))
            .all(db)
            .await
    }

    /// Revoke a refresh token by id (mark as revoked). Returns the updated model.
    pub async fn revoke_by_id(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<refresh_token::Model, DbErr> {
        if let Some(model) = RefreshToken::find_by_id(id).one(db).await? {
            let mut active: refresh_token::ActiveModel = model.into();
            active.revoked = Set(true);
            let updated = active.update(db).await?;
            Ok(updated)
        } else {
            Err(DbErr::RecordNotFound(format!(
                "refresh token {} not found",
                id
            )))
        }
    }

    /// Revoke all refresh tokens for a given user. Returns the number of tokens revoked.
    pub async fn revoke_by_user(db: &DatabaseConnection, user_id: Uuid) -> Result<u64, DbErr> {
        let tokens = RefreshToken::find()
            .filter(refresh_token::Column::UserId.eq(user_id))
            .all(db)
            .await?;

        let mut count: u64 = 0;
        for t in tokens.into_iter() {
            let mut active: refresh_token::ActiveModel = t.into();
            // `ActiveValue<T>` provides `unwrap()` to extract the inner value.
            // Use `unwrap()` here to obtain the current `bool` for `revoked`.
            if !active.revoked.clone().unwrap() {
                active.revoked = Set(true);
                let _ = active.update(db).await?;
                count += 1;
            }
        }

        Ok(count)
    }

    /// Delete expired refresh tokens. Returns number of deleted rows.
    ///
    /// Note: This is a simple cleanup implemented in Rust to avoid DB-specific datetime
    /// comparisons. For large datasets consider a DB-side query to delete expired rows.
    pub async fn delete_expired(db: &DatabaseConnection) -> Result<u64, DbErr> {
        let tokens = RefreshToken::find().all(db).await?;
        let mut deleted: u64 = 0;
        let now_ts = Utc::now().timestamp();

        for t in tokens.into_iter() {
            if let Some(exp) = t.expires_at {
                // DateTimeWithTimeZone should expose `timestamp()` like chrono types.
                // If using a different datetime type, adjust accordingly.
                if exp.timestamp() < now_ts {
                    let res = RefreshToken::delete_by_id(t.id).exec(db).await?;
                    deleted += res.rows_affected;
                }
            }
        }

        Ok(deleted)
    }
}
