use crate::api::users::dto::{UpdateUser, UserResponse};
use crate::api::users::repository::UserRepository;
use crate::shared::errors::api_errors::ApiError;
use crate::shared::models::users::user;
use crate::shared::models::users::user::ActiveModel;
use crate::shared::utils::auth_utils::verify_password;
use crate::shared::utils::validation::{is_unique_violation, is_valid_email};
use chrono::Utc;
use sea_orm::{DatabaseConnection, Set};
use uuid::Uuid;

pub struct UserService;

impl UserService {
    /// Authenticate a user. Returns the `user_model` on success — the handler
    /// is responsible for issuing tokens after creating the refresh token.
    pub async fn login(
        db: &DatabaseConnection,
        email: &str,
        password: &str,
    ) -> Result<user::Model, ApiError> {
        if email.trim().is_empty() || password.is_empty() {
            return Err(ApiError::BadRequest("Email and password must be provided".into()));
        }

        let user = UserRepository::find_by_email(db, email)
            .await
            .map_err(|e| ApiError::InternalError(e.to_string()))?
            .ok_or_else(|| ApiError::Unauthorized("Invalid credentials".into()))?;

        if !user.is_active {
            return Err(ApiError::Unauthorized("Invalid credentials".into()));
        }

        if !verify_password(&user.password_hash, password)? {
            return Err(ApiError::Unauthorized("Invalid credentials".into()));
        }

        let active = ActiveModel {
            id: Set(user.id),
            last_login: Set(Some(Utc::now().into())),
            ..Default::default()
        };
        let updated = UserRepository::update(db, active)
            .await
            .map_err(|e| ApiError::InternalError(format!("DB update failed: {}", e)))?;

        Ok(updated)
    }

    pub async fn list_users(
        db: &DatabaseConnection,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<UserResponse>, ApiError> {
        let users = UserRepository::find_all(db, limit, offset)
            .await
            .map_err(|_| ApiError::InternalError("DB error".to_string()))?;

        Ok(users
            .into_iter()
            .map(|m| UserResponse { id: m.id, name: m.name, email: m.email })
            .collect())
    }

    pub async fn get_user(db: &DatabaseConnection, id: Uuid) -> Result<UserResponse, ApiError> {
        let user = UserRepository::find_by_id(db, id)
            .await
            .map_err(|_| ApiError::InternalError("DB error".to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("User {} not found", id)))?;

        Ok(UserResponse { id: user.id, name: user.name, email: user.email })
    }

    /// Update a user. `caller_id` must match `id` — users can only update themselves.
    pub async fn update_user(
        db: &DatabaseConnection,
        caller_id: Uuid,
        id: Uuid,
        input: UpdateUser,
    ) -> Result<UserResponse, ApiError> {
        if caller_id != id {
            return Err(ApiError::Unauthorized("Cannot modify another user's account".into()));
        }

        let existing = UserRepository::find_by_id(db, id)
            .await
            .map_err(|_| ApiError::InternalError("DB error".to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("User {} not found", id)))?;

        let mut active: ActiveModel = existing.into();

        if let Some(name) = input.name {
            if name.trim().is_empty() {
                return Err(ApiError::BadRequest("Name cannot be empty".into()));
            }
            active.name = Set(name);
        }

        if let Some(email) = input.email {
            if !is_valid_email(&email) {
                return Err(ApiError::BadRequest("Invalid email address".into()));
            }
            active.email = Set(email);
        }

        let updated = UserRepository::update(db, active).await.map_err(|e| {
            if is_unique_violation(&e) {
                ApiError::Conflict("Email already exists".into())
            } else {
                ApiError::InternalError("DB update failed".to_string())
            }
        })?;

        Ok(UserResponse { id: updated.id, name: updated.name, email: updated.email })
    }

    /// Delete a user. `caller_id` must match `id` — users can only delete themselves.
    pub async fn delete_user(
        db: &DatabaseConnection,
        caller_id: Uuid,
        id: Uuid,
    ) -> Result<(), ApiError> {
        if caller_id != id {
            return Err(ApiError::Unauthorized("Cannot delete another user's account".into()));
        }
        let rows = UserRepository::delete(db, id)
            .await
            .map_err(|_| ApiError::InternalError("DB delete failed".to_string()))?;

        if rows == 0 {
            return Err(ApiError::NotFound(format!("User {} not found", id)));
        }
        Ok(())
    }
}
