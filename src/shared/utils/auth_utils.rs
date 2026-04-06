use crate::shared::config::load_env_var::JwtConfig;
use crate::shared::errors::api_errors::ApiError;
use argon2::{
    Argon2,
    password_hash::{
        PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
        rand_core::{OsRng, RngCore},
    },
};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, TokenData, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// JWT claims used in access tokens.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    /// Token version for revocation. Optional so tokens issued without a version
    /// skip the version check; presence triggers it.
    pub tv: Option<i32>,
}

/// Hash a plaintext password using Argon2id.
/// The returned PHC string includes salt and parameters and can be stored directly in the DB.
pub fn hash_password(password: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|_| ApiError::InternalError("Password hashing failed".into()))
}

/// Verify a plaintext password against a stored Argon2 hash.
/// Returns `Ok(true)` on match, `Ok(false)` on mismatch.
pub fn verify_password(hash: &str, password: &str) -> Result<bool, ApiError> {
    let parsed = PasswordHash::new(hash)
        .map_err(|_| ApiError::InternalError("Invalid password hash".into()))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

/// Create a signed HS256 JWT access token for `user_id`.
pub fn create_jwt(user_id: Uuid, token_version: Option<i32>, cfg: &JwtConfig) -> Result<String, ApiError> {
    let exp = (Utc::now() + Duration::minutes(cfg.access_exp_minutes)).timestamp() as usize;
    let claims = Claims { sub: user_id.to_string(), exp, tv: token_version };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(cfg.secret.as_ref()),
    )
    .map_err(|_| ApiError::InternalError("Token creation failed".into()))
}

/// Decode and validate a JWT access token.
pub fn decode_jwt(token: &str, cfg: &JwtConfig) -> Result<TokenData<Claims>, ApiError> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(cfg.secret.as_ref()),
        &Validation::default(),
    )
    .map_err(|e| ApiError::BadRequest(format!("Invalid token: {}", e)))
}

/// Generate a cryptographically secure 128-character hex-encoded opaque refresh token.
pub fn generate_refresh_token() -> String {
    let mut bytes = [0u8; 64];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().fold(String::with_capacity(128), |mut s, b| {
        s.push_str(&format!("{:02x}", b));
        s
    })
}

/// Compute the refresh token expiry as a Unix timestamp.
pub fn refresh_expiry_timestamp(cfg: &JwtConfig) -> i64 {
    (Utc::now() + Duration::days(cfg.refresh_exp_days)).timestamp()
}
