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

/// Typed error for the token rotation transaction — avoids string matching on `TransactionError`.
#[derive(Debug)]
enum RotateError {
    TokenNotFound,
    UserNotFound,
    Db(sea_orm::DbErr),
    Jwt(String),
}

impl From<sea_orm::DbErr> for RotateError {
    fn from(e: sea_orm::DbErr) -> Self {
        RotateError::Db(e)
    }
}

impl std::fmt::Display for RotateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RotateError::TokenNotFound => write!(f, "Invalid or expired refresh token"),
            RotateError::UserNotFound => write!(f, "User not found"),
            RotateError::Db(e) => write!(f, "DB error: {}", e),
            RotateError::Jwt(e) => write!(f, "JWT error: {}", e),
        }
    }
}

pub struct AuthService;

impl AuthService {
    /// Authenticate a user, update last_login, and create a refresh token atomically.
    /// Returns `(user_model, refresh_token_plaintext)`.
    pub async fn login(
        db: &DatabaseConnection,
        email: &str,
        password: &str,
    ) -> Result<(user::Model, String), ApiError> {
        use crate::shared::utils::auth_utils::verify_password;

        if email.trim().is_empty() || password.is_empty() {
            return Err(ApiError::BadRequest("Email and password must be provided".into()));
        }

        // Fetch and verify credentials outside the transaction — Argon2 is CPU-intensive
        let user = UserRepository::find_by_email(db, email)
            .await
            .map_err(|e| ApiError::InternalError(e.to_string()))?
            .ok_or_else(|| ApiError::Unauthorized("Invalid credentials".into()))?;

        if !user.is_active {
            return Err(ApiError::Unauthorized("Invalid credentials".into()));
        }
        if !verify_password(&user.password_hash, &password)? {
            return Err(ApiError::Unauthorized("Invalid credentials".into()));
        }

        let cfg = JwtConfig::get();
        let plain = generate_refresh_token();
        let expires_at = Some(DateTimeWithTimeZone::from(
            timestamp_to_datetime(refresh_expiry_timestamp(cfg))?,
        ));
        let user_id = user.id;
        let plain_for_txn = plain.clone();

        // Update last_login and create refresh token atomically
        let user_model = db
            .transaction::<_, user::Model, sea_orm::DbErr>(|txn| {
                Box::pin(async move {
                    use crate::shared::models::users::user::ActiveModel;
                    use chrono::Utc;
                    use sea_orm::ActiveModelTrait;
                    let active = ActiveModel {
                        id: Set(user_id),
                        last_login: Set(Some(Utc::now().into())),
                        ..Default::default()
                    };
                    let updated = active.update(txn).await?;
                    RefreshTokenRepository::create(txn, user_id, plain_for_txn, expires_at).await?;
                    Ok(updated)
                })
            })
            .await
            .map_err(|e| ApiError::InternalError(format!("Login session creation failed: {}", e)))?;

        Ok((user_model, plain))
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
                    let model = UserRepository::insert(txn, active).await?;
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
            .transaction::<_, String, RotateError>(|txn| {
                Box::pin(async move {
                    let record = RefreshTokenRepository::find_active_by_token_hash(txn, &incoming_hash)
                        .await
                        .map_err(RotateError::Db)?
                        .ok_or(RotateError::TokenNotFound)?;

                    let user = UserRepository::find_by_id(txn, record.user_id)
                        .await
                        .map_err(RotateError::Db)?
                        .ok_or(RotateError::UserNotFound)?;

                    let token = create_jwt(record.user_id, Some(user.token_version), JwtConfig::get())
                        .map_err(|e| RotateError::Jwt(e.to_string()))?;

                    RefreshTokenRepository::create(txn, record.user_id, new_plain_for_txn, new_expires_at)
                        .await
                        .map_err(RotateError::Db)?;
                    RefreshTokenRepository::revoke_by_id(txn, record.id)
                        .await
                        .map_err(RotateError::Db)?;

                    Ok(token)
                })
            })
            .await
            .map_err(|e| match e {
                sea_orm::TransactionError::Transaction(RotateError::TokenNotFound) => {
                    ApiError::Unauthorized("Invalid or expired refresh token".into())
                }
                sea_orm::TransactionError::Transaction(RotateError::UserNotFound) => {
                    ApiError::NotFound("User not found".into())
                }
                e => ApiError::InternalError(format!("Token rotation failed: {}", e)),
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
