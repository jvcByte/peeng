use crate::api::refresh_tokens::repository::RefreshTokenRepository;
use crate::api::users::repository::UserRepository;
use crate::shared::config::load_env_var::JwtConfig;
use crate::shared::errors::api_errors::ApiError;
use crate::shared::models::users::user;
use crate::shared::utils::auth_utils::{
    create_jwt, generate_refresh_token, hash_password, refresh_expiry_timestamp,
    timestamp_to_datetime,
};
use chrono::Utc;
use sea_orm::prelude::DateTimeWithTimeZone;
use sea_orm::{DatabaseConnection, Set, TransactionTrait};
use uuid::Uuid;

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
    /// If either the user insert or the token insert fails, both are rolled back.
    pub async fn register(
        db: &DatabaseConnection,
        name: String,
        email: String,
        password: String,
    ) -> Result<(user::Model, String), ApiError> {
        use crate::shared::models::users::user::ActiveModel;
        // Validate before entering the transaction
        if name.trim().is_empty() {
            return Err(ApiError::BadRequest("Name cannot be empty".into()));
        }
        if !is_valid_email(&email) {
            return Err(ApiError::BadRequest("Invalid email address".into()));
        }
        if password.chars().count() < 8 {
            return Err(ApiError::BadRequest("Password must be at least 8 characters".into()));
        }

        let password_hash = hash_password(&password)?;
        let id = Uuid::new_v4();
        let cfg = JwtConfig::get();
        let plain = generate_refresh_token();
        let expires_at = Some(DateTimeWithTimeZone::from(
            timestamp_to_datetime(refresh_expiry_timestamp(cfg))?,
        ));

        let plain_clone = plain.clone();

        let user_model = db
            .transaction::<_, user::Model, sea_orm::DbErr>(|txn| {
                let name = name.clone();
                let email = email.clone();
                let password_hash = password_hash.clone();
                let plain = plain_clone.clone();
                let expires_at = expires_at.clone();
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
                    RefreshTokenRepository::create(txn, id, plain, expires_at).await?;
                    Ok(model)
                })
            })
            .await
            .map_err(|e| {
                // TransactionError wraps the inner DbErr — check both
                let msg = e.to_string().to_lowercase();
                if msg.contains("unique") || msg.contains("duplicate") || msg.contains("23505") {
                    ApiError::Conflict("Email already exists".into())
                } else {
                    ApiError::InternalError(format!("Registration failed: {}", e))
                }
            })?;

        Ok((user_model, plain))
    }

    /// Verify a refresh token by hash, rotate it, and return a new access token + new refresh token.
    /// The user fetch, token create, and token revoke are all inside a transaction so
    /// token_version is read consistently and no orphaned tokens are left on failure.
    pub async fn verify_and_rotate_refresh(
        db: &DatabaseConnection,
        incoming_plain: &str,
    ) -> Result<(String, String), ApiError> {
        let record = RefreshTokenRepository::find_active_by_token(db, incoming_plain)
            .await
            .map_err(|e| ApiError::InternalError(e.to_string()))?
            .ok_or_else(|| ApiError::Unauthorized("Invalid or expired refresh token".into()))?;

        let cfg = JwtConfig::get();
        let new_plain = generate_refresh_token();
        let new_expires_at = Some(DateTimeWithTimeZone::from(
            timestamp_to_datetime(refresh_expiry_timestamp(cfg))?,
        ));

        let new_plain_clone = new_plain.clone();
        let old_id = record.id;
        let user_id = record.user_id;

        // Read token_version and perform rotation atomically
        let access_token = db
            .transaction::<_, String, sea_orm::DbErr>(|txn| {
                Box::pin(async move {
                    // Read token_version inside the transaction for consistency
                    let user = UserRepository::find_by_id(txn, user_id)
                        .await?
                        .ok_or_else(|| sea_orm::DbErr::RecordNotFound("User not found".into()))?;

                    let token = create_jwt(user_id, Some(user.token_version), JwtConfig::get())
                        .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;

                    RefreshTokenRepository::create(txn, user_id, new_plain_clone, new_expires_at)
                        .await?;
                    RefreshTokenRepository::revoke_by_id(txn, old_id).await?;

                    Ok(token)
                })
            })
            .await
            .map_err(|e| ApiError::InternalError(format!("Token rotation failed: {}", e)))?;

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

    /// Revoke all refresh tokens for a user (global logout). Returns number revoked.
    pub async fn revoke_all_for_user(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> Result<u64, ApiError> {
        RefreshTokenRepository::revoke_by_user(db, user_id)
            .await
            .map_err(|e| ApiError::InternalError(format!("DB error revoking tokens: {}", e)))
    }
}

fn is_valid_email(email: &str) -> bool {
    let email = email.trim();
    if let Some((local, domain)) = email.split_once('@') {
        !local.is_empty()
            && domain.contains('.')
            && !domain.starts_with('.')
            && !domain.ends_with('.')
    } else {
        false
    }
}
