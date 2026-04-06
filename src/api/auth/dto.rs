// crud-with-sea_orm/src/api/auth/dto.rs
//! Authentication-related DTOs: requests and responses used by the auth endpoints.

use serde::{Deserialize, Serialize};

/// Request payload for user registration.
///
/// Example:
/// {
///   "name": "Alice",
///   "email": "alice@example.com",
///   "password": "s3cureP@ssw0rd"
/// }
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub name: String,
    pub email: String,
    pub password: String,
}

/// Request payload for login.
///
/// Example:
/// {
///   "email": "alice@example.com",
///   "password": "s3cureP@ssw0rd"
/// }
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// Request payload used to refresh an access token.
///
/// Clients should present the refresh token they previously received.
/// Example:
/// {
///   "refresh_token": "..."
/// }
#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

/// Response returned when issuing tokens (access token, optional refresh token).
///
/// - `access_token`: short-lived JWT (use in Authorization: Bearer ...)
/// - `token_type`: typically "Bearer"
/// - `expires_in`: lifetime of the access token in seconds
/// - `refresh_token`: opaque refresh token (present when issuing/rotating)
/// - `user`: optional public user info (if included by the endpoint)
#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<crate::api::users::dto::UserResponse>,
}
