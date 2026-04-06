use crate::api::auth::dto::{LoginRequest, RefreshRequest, RegisterRequest, TokenResponse};
use crate::api::auth::service::AuthService;
use crate::api::users::dto::UserResponse;
use crate::api::users::service::UserService;
use crate::shared::config::app_state::AppState;
use crate::shared::config::load_env_var::JwtConfig;
use crate::shared::errors::api_errors::ApiError;
use crate::shared::middleware::auth::AuthenticatedUser;
use crate::shared::models::users::user;
use crate::shared::utils::auth_utils::create_jwt;
use actix_web::{HttpResponse, Result, web};
use serde_json::json;

/// Build a `TokenResponse` from a user model and a plaintext refresh token.
fn build_token_response(
    user_model: &user::Model,
    refresh_plain: String,
    include_user: bool,
) -> Result<TokenResponse, ApiError> {
    let cfg = JwtConfig::get();
    let access_token = create_jwt(user_model.id, Some(user_model.token_version), cfg)?;
    Ok(TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in: cfg.access_exp_minutes * 60,
        refresh_token: Some(refresh_plain),
        user: if include_user {
            Some(UserResponse {
                id: user_model.id,
                name: user_model.name.clone(),
                email: user_model.email.clone(),
            })
        } else {
            None
        },
    })
}

/// Register a new user and return tokens + user info.
/// User creation and refresh token creation are atomic — no orphaned accounts.
pub async fn register(
    body: web::Json<RegisterRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let req = body.into_inner();
    let (user_model, refresh_plain) =
        AuthService::register(&state.db, req.name, req.email, req.password).await?;
    let resp = build_token_response(&user_model, refresh_plain, true)?;
    Ok(HttpResponse::Created().json(resp))
}

/// Login and return tokens + user info.
pub async fn login(
    body: web::Json<LoginRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let req = body.into_inner();
    let user_model = UserService::login(&state.db, &req.email, &req.password).await?;
    let refresh_plain = AuthService::create_refresh_for_user(&state.db, user_model.id).await?;
    let resp = build_token_response(&user_model, refresh_plain, true)?;
    Ok(HttpResponse::Ok().json(resp))
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
    Ok(HttpResponse::Ok().json(TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in: cfg.access_exp_minutes * 60,
        refresh_token: Some(new_refresh_plain),
        user: None,
    }))
}

/// Revoke a specific refresh token (single-device logout).
/// Other sessions remain valid — use logout_all to invalidate everything.
pub async fn logout(
    body: web::Json<RefreshRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let req = body.into_inner();
    AuthService::revoke_refresh_token(&state.db, &req.refresh_token).await?;
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

/// Revoke all sessions and invalidate all access tokens (global logout).
pub async fn logout_all(
    user: AuthenticatedUser,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let count = AuthService::revoke_all_for_user(&state.db, user.id).await?;
    UserService::increment_token_version(&state.db, user.id).await?;
    Ok(HttpResponse::Ok().json(json!({ "message": "All sessions revoked", "count": count })))
}
