// Authentication handlers
use crate::api::auth::dto::{LoginRequest, RefreshRequest, RegisterRequest, TokenResponse};
use crate::api::auth::repository::UserRepository;
use crate::api::auth::service::AuthService;
use crate::api::users::dto::{CreateUser, UserResponse};
use crate::api::users::service::UserService;
use crate::shared::config::app_state::AppState;
use crate::shared::config::load_env_var::JwtConfig;
use crate::shared::errors::api_errors::ApiError;
use crate::shared::middleware::auth::AuthenticatedUser;
use crate::shared::utils::auth_utils::create_jwt;
use actix_web::{HttpResponse, Result, web};
use serde_json::json;

/// Register a new user and return an access token, refresh token and user info.
///
/// This refactored handler delegates refresh token creation to `AuthService` and
/// uses `UserService` to perform registration and authentication.
pub async fn register(
    body: web::Json<RegisterRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let req = body.into_inner();

    // Build the CreateUser DTO used by existing service logic.
    let create = CreateUser {
        name: req.name.clone(),
        email: req.email.clone(),
    };

    // Create the user (this hashes password and persists auth fields).
    let id = UserService::register_user(&state.db, create, req.password.clone())
        .await
        .map_err(|e| e)?;

    let cfg = JwtConfig::get();

    // Generate an access token
    let access_token = create_jwt(id, 0, &cfg)
        .map_err(|e| ApiError::InternalError(format!("Token generation error: {}", e)))?;

    let expires_in = cfg.access_exp_minutes * 60;

    // Create and persist refresh token via AuthService (returns the plain token)
    let refresh_plain = AuthService::create_refresh_for_user(&state.db, id)
        .await
        .map_err(|e| e)?;

    // Fetch created user to include public user info in the response.
    let user_model = UserRepository::find_by_id(&state.db, id)
        .await
        .map_err(|err| ApiError::InternalError(format!("DB Error: {}", err)))?
        .ok_or_else(|| ApiError::NotFound("User Not Found".into()))?;

    let user = UserResponse {
        id: user_model.id,
        name: user_model.name,
        email: user_model.email,
    };

    // Build response
    Ok(HttpResponse::Created().json(TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in,
        refresh_token: Some(refresh_plain),
        user: Some(user),
    }))
}

/// Login an existing user and return an access token and refresh token and user info.
pub async fn login(
    body: web::Json<LoginRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let req = body.into_inner();

    // Authenticate credentials and create access token (service returns token string).
    let access_token = UserService::login(&state.db, &req.email, &req.password)
        .await
        .map_err(|e| e)?;

    // Retrieve user public info
    let user_model = UserRepository::find_by_email(&state.db, &req.email)
        .await
        .map_err(|e| ApiError::InternalError(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("User Not Found".into()))?;

    let user = UserResponse {
        id: user_model.id,
        name: user_model.name,
        email: user_model.email,
    };

    // Create and persist refresh token (delegated to AuthService)
    let refresh_plain = AuthService::create_refresh_for_user(&state.db, user_model.id)
        .await
        .map_err(|e| e)?;

    // Compute expiry from config (expiry is inlined in the response below)
    let cfg = JwtConfig::get();
    let expires_in = cfg.access_exp_minutes * 60;

    Ok(HttpResponse::Ok().json(TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in: expires_in,
        refresh_token: Some(refresh_plain),
        user: Some(user),
    }))
}

/// Refresh access token using a refresh token.
///
/// Delegates verification & rotation to `AuthService`.
pub async fn refresh(
    body: web::Json<RefreshRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let req = body.into_inner();

    let (access_token, new_refresh_plain) =
        AuthService::verify_and_rotate_refresh(&state.db, &req.refresh_token)
            .await
            .map_err(|e| e)?;

    let cfg = JwtConfig::get();
    let expires_in = cfg.access_exp_minutes * 60;

    Ok(HttpResponse::Ok().json(TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in: expires_in,
        refresh_token: Some(new_refresh_plain),
        user: None,
    }))
}

/// Logout and revoke refresh tokens / session.
/// Client should present the refresh token to revoke it server-side.
/// This also increments the user's token_version to invalidate all access tokens.
pub async fn logout(
    body: web::Json<RefreshRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let req = body.into_inner();

    // Revoke the refresh token and obtain the associated user id
    let user_id = AuthService::revoke_refresh_token(&state.db, &req.refresh_token)
        .await
        .map_err(|e| e)?;

    // Increment user's token_version to invalidate all access tokens
    UserService::increment_token_version(&state.db, user_id)
        .await
        .map_err(|_| ApiError::InternalError("Failed to invalidate tokens".into()))?;

    Ok(HttpResponse::NoContent().finish())
}

/// Return the currently authenticated user's public information.
pub async fn me(user: AuthenticatedUser) -> Result<HttpResponse, ApiError> {
    let resp = UserResponse {
        id: user.id,
        name: user.name,
        email: user.email,
    };
    Ok(HttpResponse::Ok().json(resp))
}

/// Revoke all refresh tokens for the authenticated user (global logout).
pub async fn logout_all(
    user: AuthenticatedUser,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let count = AuthService::revoke_all_for_user(&state.db, user.id)
        .await
        .map_err(|e| e)?;

    // Increment token version to invalidate all access tokens
    UserService::increment_token_version(&state.db, user.id)
        .await
        .map_err(|e| ApiError::InternalError(e.to_string()))?;

    Ok(HttpResponse::Ok().json(json!({
        "message": "All sessions revoked",
        "count": count
    })))
}

/// Admin endpoint to clean up expired refresh tokens.
pub async fn cleanup_expired_tokens(state: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    let deleted = AuthService::cleanup_expired(&state.db)
        .await
        .map_err(|e| e)?;

    Ok(HttpResponse::Ok().json(json!({
        "message": "Expired tokens cleaned up",
        "deleted": deleted
    })))
}
