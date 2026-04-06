use crate::api::refresh_tokens::repository::RefreshTokenRepository;
use crate::api::users::repository::UserRepository;
use crate::shared::config::load_env_var::JwtConfig;
use crate::shared::errors::api_errors::ApiError;
use crate::shared::models::users::user;
use crate::shared::utils::auth_utils::{
    create_jwt, generate_refresh_token, hash_password, hash_token, refresh_expiry_timestamp,
    timestamp_to_datetime,
};
use crate::shared::utils::validation::{is_unique_violation_str, is_valid_email};
use chrono::Utc;
use sea_orm::prelude::DateTimeWithTimeZone;
use sea_orm::{DatabaseConnection, Set, TransactionTrait};
use uuid::Uuid;

/// Sentinel used to propagate a typed "token not found" error out of a transaction
/// without relying on fragile string matching.
const TOKEN_NOT_FOUND: &str = "TOKEN_NOT_FOUND";

pub struct AuthService;

impl AuthService {
    /// Create a new refresh token for a user and return the plaintext token.
    pub async fn create_refresh_for_user(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> Result<String, ApiError> {
        let cfg = JwtConfig::get();
        let plain = generate_refresh_token();
        let expires_at = Some(DateTimeWithTimeZone::from(
            timestamp_to_datetime(refresh_expiry_timestamp(cfg))?,
        ));
        RefreshTokenRepository::create(db, user_id, plain.clone(), expires_at)
            .await
            .map_err(|e| ApiError::InternalError(format!("DB error storing refresh token: {}", e)))?;
        Ok(plain)
    }

    /// Register a user and create their first refresh token atomically.
    /// Returns `(user_model, refresh_token_plaintext)`.
    ///
    /// Note: `hash_password` (Argon2, CPU-intensive) is called *before* the transaction
    /// to avoid holding the DB connection open during hashing.
    pub async fn register(
        db: &DatabaseConnection,
        name: String,
        email: String,
        password: String,
    ) -> Result<(user::Model, String), ApiError> {
        use crate::shared::models::users::user::ActiveModel;

        if name.trim().is_empty() {
            return Err(ApiError::BadRequest("Name cannot be empty".into()));
        }
        if !is_valid_email(&email) {
            return Err(ApiError::BadRequest("Invalid email address".into()));
        }
        if password.chars().count() < 8 {
            return Err(ApiError::BadRequest("Password must be at least 8 characters".into()));
        }

        // Hash password before the transaction — Argon2 is CPU-intensive and should
        // not hold a DB connection open while running.
        let password_hash = hash_password(&password)?;
        let id = Uuid::new_v4();
        let cfg = JwtConfig::get();
        let plain = generate_refresh_token();
        let expires_at = Some(DateTimeWithTimeZone::from(
            timestamp_to_datetime(refresh_expiry_timestamp(cfg))?,
        ));

        let plain_for_txn = plain.clone();

        let user_model = db
            .transaction::<_, user::Model, sea_orm::DbErr>(|txn| {
                Box::pin(async move {
                    use sea_orm::EntityTrait;
                    let active = ActiveModel {
                        id: Set(id),
                        name: Set(name),
                        email: Set(email),
                        password_hash: Set(password_hash),
                        is_active: Set(true),
                        token_version: Set(0),
                        created_at: Set(Some(Utc::now().into())),
                        ..Default::default()
                    };
                    let model = crate::shared::models::users::User::insert(active)
                        .exec_with_returning(txn)
                        .await?;
                    RefreshTokenRepository::create(txn, id, plain_for_txn, expires_at).await?;
                    Ok(model)
                })
            })
            .await
            .map_err(|e| {
                if is_unique_violation_str(&e.to_string()) {
                    ApiError::Conflict("Email already exists".into())
                } else {
                    ApiError::InternalError(format!("Registration failed: {}", e))
                }
            })?;

        Ok((user_model, plain))
    }

    /// Verify a refresh token by hash, rotate it, and return a new access token + new refresh token.
    /// The lookup, user fetch, token create, and token revoke are all inside a single transaction
    /// — eliminates the TOCTOU race and ensures no orphaned tokens on failure.
    pub async fn verify_and_rotate_refresh(
        db: &DatabaseConnection,
        incoming_plain: &str,
    ) -> Result<(String, String), ApiError> {
        let cfg = JwtConfig::get();
        let new_plain = generate_refresh_token();
        let new_expires_at = Some(DateTimeWithTimeZone::from(
            timestamp_to_datetime(refresh_expiry_timestamp(cfg))?,
        ));

        // Hash before the transaction so the closure is `'static`-compatible
        let incoming_hash = hash_token(incoming_plain);
        let new_plain_for_txn = new_plain.clone();

        let access_token = db
            .transaction::<_, String, sea_orm::DbErr>(|txn| {
                Box::pin(async move {
                    let record = RefreshTokenRepository::find_active_by_token_hash(txn, &incoming_hash)
                        .await?
                        .ok_or_else(|| {
                            // Use a typed sentinel — avoids fragile string matching on the outside
                            sea_orm::DbErr::Custom(TOKEN_NOT_FOUND.to_string())
                        })?;

                    let user = UserRepository::find_by_id(txn, record.user_id)
                        .await?
                        .ok_or_else(|| sea_orm::DbErr::RecordNotFound("User not found".into()))?;

                    let token = create_jwt(record.user_id, Some(user.token_version), JwtConfig::get())
                        .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;

                    RefreshTokenRepository::create(txn, record.user_id, new_plain_for_txn, new_expires_at)
                        .await?;
                    RefreshTokenRepository::revoke_by_id(txn, record.id).await?;

                    Ok(token)
                })
            })
            .await
            .map_err(|e| {
                // Match on the typed sentinel — no fragile substring search
                if e.to_string().contains(TOKEN_NOT_FOUND) {
                    ApiError::Unauthorized("Invalid or expired refresh token".into())
                } else {
                    ApiError::InternalError(format!("Token rotation failed: {}", e))
                }
            })?;

        Ok((access_token, new_plain))
    }

    /// Revoke a specific refresh token (single-device logout).
    pub async fn revoke_refresh_token(
        db: &DatabaseConnection,
        incoming_plain: &str,
    ) -> Result<(), ApiError> {
        let record = RefreshTokenRepository::find_active_by_token(db, incoming_plain)
            .await
            .map_err(|_| ApiError::InternalError("DB error".to_string()))?
            .ok_or_else(|| ApiError::Unauthorized("Invalid or expired refresh token".into()))?;

        RefreshTokenRepository::revoke_by_id(db, record.id)
            .await
            .map_err(|_| ApiError::InternalError("Failed to revoke refresh token".into()))?;

        Ok(())
    }

    /// Revoke all refresh tokens and invalidate all access tokens atomically (global logout).
    pub async fn revoke_all_and_invalidate(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> Result<(), ApiError> {
        db.transaction::<_, (), sea_orm::DbErr>(|txn| {
            Box::pin(async move {
                RefreshTokenRepository::revoke_by_user(txn, user_id).await?;
                UserRepository::increment_token_version(txn, user_id).await?;
                Ok(())
            })
        })
        .await
        .map_err(|e| ApiError::InternalError(format!("Global logout failed: {}", e)))
    }
}
