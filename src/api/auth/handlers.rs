use crate::api::auth::dto::{LoginRequest, RefreshRequest, RegisterRequest, TokenResponse};
use crate::api::auth::service::AuthService;
use crate::api::users::dto::UserResponse;
use crate::api::users::service::UserService;
use crate::shared::config::app_state::AppState;
use crate::shared::config::load_env_var::JwtConfig;
use crate::shared::errors::api_errors::ApiError;
use crate::shared::middleware::auth::AuthenticatedUser;
use crate::shared::utils::auth_utils::create_jwt;
use actix_web::{HttpResponse, Result, web};
use serde_json::json;

/// Register a new user and return tokens + user info.
pub async fn register(
    body: web::Json<RegisterRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let req = body.into_inner();

    let (id, user_model) = UserService::register_user(
        &state.db,
        req.name,
        req.email,
        req.password,
    )
    .await?;

    // Create refresh token first — embed its token_version in the access token
    // so both are consistent from the start and no valid access token exists
    // without a corresponding session.
    let (refresh_plain, token_version) = AuthService::create_refresh_for_user(&state.db, id).await?;

    let cfg = JwtConfig::get();
    let access_token = create_jwt(id, Some(token_version), cfg)?;
    let expires_in = cfg.access_exp_minutes * 60;

    Ok(HttpResponse::Created().json(TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in,
        refresh_token: Some(refresh_plain),
        user: Some(UserResponse {
            id: user_model.id,
            name: user_model.name,
            email: user_model.email,
        }),
    }))
}

/// Login and return tokens + user info.
pub async fn login(
    body: web::Json<LoginRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let req = body.into_inner();

    // Authenticate — returns user model only, no token yet
    let user_model = UserService::login(&state.db, &req.email, &req.password).await?;

    // Create refresh token first — embed its token_version in the access token
    // so both are consistent and the middleware version check passes immediately
    let (refresh_plain, token_version) =
        AuthService::create_refresh_for_user(&state.db, user_model.id).await?;

    let cfg = JwtConfig::get();
    let access_token = create_jwt(user_model.id, Some(token_version), cfg)?;
    let expires_in = cfg.access_exp_minutes * 60;

    Ok(HttpResponse::Ok().json(TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in,
        refresh_token: Some(refresh_plain),
        user: Some(UserResponse {
            id: user_model.id,
            name: user_model.name,
            email: user_model.email,
        }),
    }))
}

/// Rotate a refresh token and return a new access token + new refresh token.
pub async fn refresh(
    body: web::Json<RefreshRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let req = body.into_inner();
    let (access_token, new_refresh_plain) =
        AuthService::verify_and_rotate_refresh(&state.db, &req.refresh_token).await?;

    let cfg = JwtConfig::get();
    let expires_in = cfg.access_exp_minutes * 60;

    Ok(HttpResponse::Ok().json(TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in,
        refresh_token: Some(new_refresh_plain),
        user: None,
    }))
}

/// Revoke a refresh token and invalidate all access tokens for the user.
pub async fn logout(
    body: web::Json<RefreshRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let req = body.into_inner();
    let user_id = AuthService::revoke_refresh_token(&state.db, &req.refresh_token).await?;
    UserService::increment_token_version(&state.db, user_id).await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Return the currently authenticated user's public info.
pub async fn me(user: AuthenticatedUser) -> Result<HttpResponse, ApiError> {
    Ok(HttpResponse::Ok().json(UserResponse {
        id: user.id,
        name: user.name,
        email: user.email,
    }))
}

/// Revoke all sessions for the authenticated user (global logout).
pub async fn logout_all(
    user: AuthenticatedUser,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let count = AuthService::revoke_all_for_user(&state.db, user.id).await?;
    UserService::increment_token_version(&state.db, user.id).await?;
    Ok(HttpResponse::Ok().json(json!({ "message": "All sessions revoked", "count": count })))
}

/// Delete expired refresh tokens. Requires authentication.
pub async fn cleanup_expired_tokens(
    _user: AuthenticatedUser,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let deleted = AuthService::cleanup_expired(&state.db).await?;
    Ok(HttpResponse::Ok().json(json!({ "message": "Expired tokens cleaned up", "deleted": deleted })))
}
