use crate::api::refresh_tokens::repository::RefreshTokenRepository;
use crate::api::users::repository::UserRepository;
use crate::shared::config::load_env_var::JwtConfig;
use crate::shared::errors::api_errors::ApiError;
use crate::shared::utils::auth_utils::{
    create_jwt, generate_refresh_token, refresh_expiry_timestamp, timestamp_to_datetime,
};
use sea_orm::DatabaseConnection;
use sea_orm::prelude::DateTimeWithTimeZone;
use uuid::Uuid;

pub struct AuthService;

impl AuthService {
    /// Create a new refresh token for a user and return the plaintext token.
    /// The `token_version` embedded in the access token must come from `users.token_version`
    /// (fetched by the caller) so it is the single source of truth.
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

    /// Verify a refresh token by hash, rotate it, and return a new access token + new refresh token.
    /// The access token's tv claim is read from `users.token_version` — the single source of truth.
    pub async fn verify_and_rotate_refresh(
        db: &DatabaseConnection,
        incoming_plain: &str,
    ) -> Result<(String, String), ApiError> {
        let record = RefreshTokenRepository::find_active_by_token(db, incoming_plain)
            .await
            .map_err(|e| ApiError::InternalError(e.to_string()))?
            .ok_or_else(|| ApiError::Unauthorized("Invalid or expired refresh token".into()))?;

        // Read token_version from users — not from the refresh token row
        let user = UserRepository::find_by_id(db, record.user_id)
            .await
            .map_err(|e| ApiError::InternalError(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound("User not found".into()))?;

        let cfg = JwtConfig::get();
        let access_token = create_jwt(record.user_id, Some(user.token_version), cfg)?;

        let new_plain = generate_refresh_token();
        let new_expires_at = Some(DateTimeWithTimeZone::from(
            timestamp_to_datetime(refresh_expiry_timestamp(cfg))?,
        ));

        // Persist new token before revoking old one — no window with zero valid sessions
        RefreshTokenRepository::create(db, record.user_id, new_plain.clone(), new_expires_at)
            .await
            .map_err(|_| ApiError::InternalError("Failed to store refresh token".into()))?;

        RefreshTokenRepository::revoke_by_id(db, record.id)
            .await
            .map_err(|_| ApiError::InternalError("Failed to revoke old refresh token".into()))?;

        Ok((access_token, new_plain))
    }

    /// Revoke a specific refresh token by hash. Returns the associated user id.
    pub async fn revoke_refresh_token(
        db: &DatabaseConnection,
        incoming_plain: &str,
    ) -> Result<Uuid, ApiError> {
        let record = RefreshTokenRepository::find_active_by_token(db, incoming_plain)
            .await
            .map_err(|_| ApiError::InternalError("DB error".to_string()))?
            .ok_or_else(|| ApiError::Unauthorized("Invalid or expired refresh token".into()))?;

        RefreshTokenRepository::revoke_by_id(db, record.id)
            .await
            .map_err(|_| ApiError::InternalError("Failed to revoke refresh token".into()))?;

        Ok(record.user_id)
    }

    /// Revoke all refresh tokens for a user. Returns number revoked.
    pub async fn revoke_all_for_user(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> Result<u64, ApiError> {
        RefreshTokenRepository::revoke_by_user(db, user_id)
            .await
            .map_err(|e| ApiError::InternalError(format!("DB error revoking tokens: {}", e)))
    }

    /// Delete expired refresh tokens. Returns number deleted.
    pub async fn cleanup_expired(db: &DatabaseConnection) -> Result<u64, ApiError> {
        RefreshTokenRepository::delete_expired(db)
            .await
            .map_err(|e| ApiError::InternalError(format!("DB error cleaning tokens: {}", e)))
    }
}
