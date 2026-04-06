use super::service::UserService;
use crate::api::users::dto::{ListUsersQuery, UpdateUser};
use crate::shared::config::app_state::AppState;
use crate::shared::errors::api_errors::ApiError;
use crate::shared::middleware::auth::AuthenticatedUser;
use actix_web::{HttpResponse, Result, web};
use uuid::Uuid;

pub async fn list_users(
    _user: AuthenticatedUser,
    query: web::Query<ListUsersQuery>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0);
    let users = UserService::list_users(&state.db, limit, offset).await?;
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
    user: AuthenticatedUser,
    path: web::Path<Uuid>,
    body: web::Json<UpdateUser>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let id = path.into_inner();
    let updated = UserService::update_user(&state.db, user.id, id, body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(updated))
}

pub async fn delete_user(
    user: AuthenticatedUser,
    path: web::Path<Uuid>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let id = path.into_inner();
    UserService::delete_user(&state.db, user.id, id).await?;
    Ok(HttpResponse::NoContent().finish())
}
