use crate::api::auth::repository::RefreshTokenRepository;
use crate::api::users::dto::{UpdateUser, UserResponse};
use crate::api::users::repository::UserRepository;
use crate::shared::config::load_env_var::JwtConfig;
use crate::shared::errors::api_errors::ApiError;
use crate::shared::models::refresh_tokens::refresh_token::ActiveModel as RefreshTokenActiveModel;
use crate::shared::models::users::user;
use crate::shared::models::users::user::ActiveModel;
use crate::shared::utils::auth_utils::{create_jwt, hash_password, verify_password};
use chrono::Utc;
use sea_orm::{DatabaseConnection, Set};
use uuid::Uuid;

pub struct UserService;

impl UserService {
    /// Register a new user. Returns `(id, user_model)` so the handler can build
    /// the response without an extra DB fetch.
    pub async fn register_user(
        db: &DatabaseConnection,
        name: String,
        email: String,
        password: String,
    ) -> Result<(Uuid, user::Model), ApiError> {
        if name.trim().is_empty() {
            return Err(ApiError::BadRequest("Name cannot be empty".into()));
        }
        if email.trim().is_empty() {
            return Err(ApiError::BadRequest("Email cannot be empty".into()));
        }
        if password.len() < 8 {
            return Err(ApiError::BadRequest("Password must be at least 8 characters".into()));
        }

        if UserRepository::find_by_email(db, &email)
            .await
            .map_err(|e| ApiError::InternalError(e.to_string()))?
            .is_some()
        {
            return Err(ApiError::Conflict("Email already exists".into()));
        }

        let password_hash = hash_password(&password)?;
        let id = Uuid::new_v4();
        let active = ActiveModel {
            id: Set(id),
            name: Set(name),
            email: Set(email),
            password_hash: Set(password_hash),
            is_active: Set(true),
            created_at: Set(Some(Utc::now().into())),
            ..Default::default()
        };

        // exec_with_returning gives us the model back — no extra fetch needed
        let model = UserRepository::insert(db, active)
            .await
            .map_err(|e| ApiError::InternalError(format!("DB insert failed: {}", e)))?;

        Ok((id, model))
    }

    /// Authenticate a user. Returns `(access_token, user_model)` so the handler
    /// can build the response without a second DB fetch.
    pub async fn login(
        db: &DatabaseConnection,
        email: &str,
        password: &str,
    ) -> Result<(String, user::Model), ApiError> {
        if email.trim().is_empty() || password.is_empty() {
            return Err(ApiError::BadRequest("Email and password must be provided".into()));
        }

        // Fetch user — generic error prevents user enumeration
        let user = UserRepository::find_by_email(db, email)
            .await
            .map_err(|e| ApiError::InternalError(e.to_string()))?
            .ok_or_else(|| ApiError::Unauthorized("Invalid credentials".into()))?;

        // Verify password before touching any session data
        if !verify_password(&user.password_hash, password)? {
            return Err(ApiError::Unauthorized("Invalid credentials".into()));
        }

        // Fetch token_version only after credentials are confirmed
        let refresh_token = RefreshTokenRepository::find_by_user_id(db, user.id)
            .await
            .map_err(|e| ApiError::InternalError(e.to_string()))?
            .ok_or_else(|| ApiError::InternalError("No session found for user".into()))?;

        let cfg = JwtConfig::get();
        let token = create_jwt(user.id, Some(refresh_token.token_version), cfg)?;

        // Update last_login
        let active = ActiveModel {
            id: Set(user.id),
            last_login: Set(Some(Utc::now().into())),
            ..Default::default()
        };
        UserRepository::update(db, active)
            .await
            .map_err(|e| ApiError::InternalError(format!("DB update failed: {}", e)))?;

        Ok((token, user))
    }

    pub async fn list_users(db: &DatabaseConnection) -> Result<Vec<UserResponse>, ApiError> {
        let users = UserRepository::find_all(db)
            .await
            .map_err(|_| ApiError::InternalError("DB error".to_string()))?;

        Ok(users
            .into_iter()
            .map(|m| UserResponse { id: m.id, name: m.name, email: m.email })
            .collect())
    }

    pub async fn get_user(db: &DatabaseConnection, id: Uuid) -> Result<UserResponse, ApiError> {
        if id == Uuid::nil() {
            return Err(ApiError::BadRequest("Invalid UUID".into()));
        }
        let user = UserRepository::find_by_id(db, id)
            .await
            .map_err(|_| ApiError::InternalError("DB error".to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("User {} not found", id)))?;

        Ok(UserResponse { id: user.id, name: user.name, email: user.email })
    }

    pub async fn update_user(
        db: &DatabaseConnection,
        id: Uuid,
        input: UpdateUser,
    ) -> Result<UserResponse, ApiError> {
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
            if email.trim().is_empty() {
                return Err(ApiError::BadRequest("Email cannot be empty".into()));
            }
            if UserRepository::find_by_email(db, &email)
                .await
                .map_err(|_| ApiError::InternalError("DB error".to_string()))?
                .filter(|u| u.id != id)
                .is_some()
            {
                return Err(ApiError::Conflict("Email already exists".into()));
            }
            active.email = Set(email);
        }

        let updated = UserRepository::update(db, active)
            .await
            .map_err(|_| ApiError::InternalError("DB update failed".to_string()))?;

        Ok(UserResponse { id: updated.id, name: updated.name, email: updated.email })
    }

    pub async fn delete_user(db: &DatabaseConnection, id: Uuid) -> Result<(), ApiError> {
        if id == Uuid::nil() {
            return Err(ApiError::BadRequest("Invalid UUID".into()));
        }
        let rows = UserRepository::delete(db, id)
            .await
            .map_err(|_| ApiError::InternalError("DB delete failed".to_string()))?;

        if rows == 0 {
            return Err(ApiError::NotFound(format!("User {} not found", id)));
        }
        Ok(())
    }

    /// Increment token_version to immediately invalidate all outstanding access tokens.
    pub async fn increment_token_version(db: &DatabaseConnection, id: Uuid) -> Result<(), ApiError> {
        let record = RefreshTokenRepository::find_by_user_id(db, id)
            .await
            .map_err(|_| ApiError::InternalError("DB error".to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("User {} not found", id)))?;

        let new_version = record.token_version + 1;
        let mut active: RefreshTokenActiveModel = record.into();
        active.token_version = Set(new_version);

        RefreshTokenRepository::update(db, active)
            .await
            .map_err(|err| ApiError::InternalError(err.to_string()))?;

        Ok(())
    }
}
