use crate::api::users::dto::{UpdateUser, UserResponse};
use crate::api::users::repository::UserRepository;
use crate::shared::errors::api_errors::ApiError;
use crate::shared::models::users::user;
use crate::shared::models::users::user::ActiveModel;
use crate::shared::utils::auth_utils::{hash_password, verify_password};
use chrono::Utc;
use sea_orm::{ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

pub struct UserService;

impl UserService {
    /// Register a new user. Returns `(id, user_model)`.
    pub async fn register_user(
        db: &DatabaseConnection,
        name: String,
        email: String,
        password: String,
    ) -> Result<(Uuid, user::Model), ApiError> {
        if name.trim().is_empty() {
            return Err(ApiError::BadRequest("Name cannot be empty".into()));
        }
        if !is_valid_email(&email) {
            return Err(ApiError::BadRequest("Invalid email address".into()));
        }
        if password.len() < 8 {
            return Err(ApiError::BadRequest("Password must be at least 8 characters".into()));
        }

        let password_hash = hash_password(&password)?;
        let id = Uuid::new_v4();
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

        let model = UserRepository::insert(db, active).await.map_err(|e| {
            // Map DB unique constraint violation to a 409 rather than a 500
            if is_unique_violation(&e) {
                ApiError::Conflict("Email already exists".into())
            } else {
                ApiError::InternalError(format!("DB insert failed: {}", e))
            }
        })?;

        Ok((id, model))
    }

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

        // Generic error on both missing user and wrong password — prevents user enumeration
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
        UserRepository::update(db, active)
            .await
            .map_err(|e| ApiError::InternalError(format!("DB update failed: {}", e)))?;

        Ok(user)
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

    /// Atomically increment users.token_version — single UPDATE with no prior SELECT.
    /// Immediately invalidates all outstanding access tokens across every session.
    pub async fn increment_token_version(db: &DatabaseConnection, id: Uuid) -> Result<(), ApiError> {
        use crate::shared::models::users::user::Column;
        use sea_orm::sea_query::Expr;

        let rows = entity::users::User::update_many()
            .col_expr(Column::TokenVersion, Expr::col(Column::TokenVersion).add(1))
            .filter(Column::Id.eq(id))
            .exec(db)
            .await
            .map_err(|e| ApiError::InternalError(e.to_string()))?
            .rows_affected;

        if rows == 0 {
            return Err(ApiError::NotFound(format!("User {} not found", id)));
        }
        Ok(())
    }
}

/// Basic email format validation — checks for a single `@` with non-empty local and domain parts.
fn is_valid_email(email: &str) -> bool {
    let email = email.trim();
    if let Some((local, domain)) = email.split_once('@') {
        !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
    } else {
        false
    }
}

/// Detect a unique constraint violation from a SeaORM `DbErr`.
fn is_unique_violation(err: &DbErr) -> bool {
    let msg = err.to_string().to_lowercase();
    msg.contains("unique") || msg.contains("duplicate") || msg.contains("23505")
}
