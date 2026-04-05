//
// Handlers
//
use super::service::UserService;
use crate::api::users::dto::UpdateUser;
use crate::shared::config::app_state::AppState;
use crate::shared::errors::api_errors::ApiError;
use crate::shared::middleware::auth::AuthenticatedUser;
use actix_web::{HttpResponse, Result, web};
use uuid::Uuid;

pub async fn list_users(
    _user: AuthenticatedUser,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let users = UserService::list_users(&state.db).await?;
    Ok(HttpResponse::Ok().json(users))
}

pub async fn get_user(
    _user: AuthenticatedUser,
    path: web::Path<Uuid>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let user = UserService::get_user(&state.db, path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(user))
}

pub async fn update_user(
    _user: AuthenticatedUser,
    path: web::Path<Uuid>,
    body: web::Json<UpdateUser>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let id = path.into_inner();
    let updated_user = UserService::update_user(&state.db, id, body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(updated_user))
}

pub async fn delete_user(
    _user: AuthenticatedUser,
    path: web::Path<Uuid>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let id = path.into_inner();
    UserService::delete_user(&state.db, id).await?;
    Ok(HttpResponse::NoContent().finish())
}
