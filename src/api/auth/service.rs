use crate::api::auth::repository::RefreshTokenRepository;
use crate::shared::config::load_env_var::JwtConfig;
use crate::shared::errors::api_errors::ApiError;
use crate::shared::utils::auth_utils::{
    create_jwt, generate_refresh_token, is_thesame, refresh_expiry_timestamp,
};
use chrono::Utc;
use sea_orm::DatabaseConnection;
use sea_orm::prelude::DateTimeWithTimeZone;
use uuid::Uuid;

/// Service layer encapsulating refresh-token and auth-related helpers.
///
/// This keeps higher-level token logic out of the HTTP handlers so the handlers
/// can remain thin and focused on request/response concerns.
pub struct AuthService;

impl AuthService {
    /// Create a new opaque refresh token, persist its hash, and return the plain token.
    pub async fn create_refresh_for_user(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> Result<String, ApiError> {
        let cfg = JwtConfig::get();
        let plain = generate_refresh_token();
        let refresh_token = plain.clone();
        // let hash = hash_refresh_token(&plain)?;
        let expires_at = Some(DateTimeWithTimeZone::from(
            chrono::DateTime::from_timestamp(refresh_expiry_timestamp(&cfg), 0)
                .ok_or_else(|| ApiError::InternalError("Failed to compute expiry".into()))?,
        ));

        RefreshTokenRepository::create(db, user_id, plain, expires_at)
            .await
            .map_err(|e| {
                ApiError::InternalError(format!("DB error storing refresh token: {}", e))
            })?;

        Ok(refresh_token)
    }

    /// Verify an incoming refresh token, rotate it (issue a new refresh token record),
    /// and return a newly minted access token + new plain refresh token.
    ///
    /// This mirrors the flow implemented in the handlers but centralizes it for reuse.
    pub async fn verify_and_rotate_refresh(
        db: &DatabaseConnection,
        incoming_plain: &str,
    ) -> Result<(String, String), ApiError> {
        // Load all active (non-revoked) tokens and find the matching one by verifying the hash.
        let all_tokens = RefreshTokenRepository::find_all_active(db)
            .await
            .map_err(|e| ApiError::InternalError(e.to_string()))?;

        let mut matching_record = None;
        for token in all_tokens {
            if let Ok(true) = is_thesame(&token.token, incoming_plain) {
                matching_record = Some(token);
                break;
            }
        }

        let record =
            matching_record.ok_or_else(|| ApiError::NotFound("Invalid refresh token".into()))?;

        if record.revoked {
            return Err(ApiError::NotFound("Refresh token revoked".into()));
        }

        if let Some(exp) = record.expires_at {
            let now_ts = Utc::now().timestamp();
            if exp.timestamp() < now_ts {
                return Err(ApiError::NotFound("Refresh token expired".into()));
            }
        }

        // Fetch user to obtain current token_version for the JWT
        let refresh_token = RefreshTokenRepository::find_by_user_id(db, record.user_id)
            .await
            .map_err(|e| ApiError::InternalError(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound("Token Not Found".into()))?;

        let cfg = JwtConfig::get();
        let access_token = create_jwt(record.user_id, refresh_token.token_version, &cfg)?;

        // Create a new refresh token and persist it, then revoke the old one.
        let new_plain = generate_refresh_token();
        let new_refresh_token = new_plain.clone();
        // let new_hash = hash_refresh_token(&new_plain)?;
        let new_expires_at = Some(DateTimeWithTimeZone::from(
            chrono::DateTime::from_timestamp(refresh_expiry_timestamp(&cfg), 0)
                .ok_or_else(|| ApiError::InternalError("Failed to compute expiry".into()))?,
        ));

        let _new_record =
            RefreshTokenRepository::create(db, record.user_id, new_plain, new_expires_at)
                .await
                .map_err(|_| ApiError::InternalError("Failed to store refresh token".into()))?;

        let _ = RefreshTokenRepository::revoke_by_id(db, record.id)
            .await
            .map_err(|_| ApiError::InternalError("Failed to revoke old refresh token".into()))?;

        Ok((access_token, new_refresh_token))
    }

    /// Revoke a refresh token presented by client (by matching the plain value).
    /// Returns the user id of the revoked token (useful if caller wants to also
    /// invalidate access tokens by bumping token_version).
    pub async fn revoke_refresh_token(
        db: &DatabaseConnection,
        incoming_plain: &str,
    ) -> Result<Uuid, ApiError> {
        let all_tokens = RefreshTokenRepository::find_all_active(db)
            .await
            .map_err(|_| ApiError::InternalError("DB error".to_string()))?;

        let mut matching_record = None;
        for token in all_tokens {
            if let Ok(true) = is_thesame(&token.token, incoming_plain) {
                matching_record = Some(token);
                break;
            }
        }

        let record =
            matching_record.ok_or_else(|| ApiError::NotFound("Invalid refresh token".into()))?;

        let _ = RefreshTokenRepository::revoke_by_id(db, record.id)
            .await
            .map_err(|_| ApiError::InternalError("Failed to revoke refresh token".into()))?;

        Ok(record.user_id)
    }

    /// Revoke all refresh tokens for the given user and return number revoked.
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
