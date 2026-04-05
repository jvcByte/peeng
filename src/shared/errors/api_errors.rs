use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use serde_json::json;
use std::fmt;

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    Conflict(String),
    NotFound(String),
    InternalError(String),
    // UnprocessableEntity(String),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::BadRequest(msg) => write!(f, "{}", msg),
            ApiError::Conflict(msg) => write!(f, "{}", msg),
            ApiError::NotFound(msg) => write!(f, "{}", msg),
            ApiError::InternalError(msg) => write!(f, "{}", msg),
            // ApiError::UnprocessableEntity(msg) => write!(f, "{}", msg),
        }
    }
}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::Conflict(_) => StatusCode::CONFLICT,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            // ApiError::UnprocessableEntity(_) => StatusCode::UNPROCESSABLE_ENTITY,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).json(json!({
            "error": self.to_string()
        }))
    }
}
