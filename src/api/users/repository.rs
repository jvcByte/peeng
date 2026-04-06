use crate::shared::models::users::{User, user};
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

pub struct UserRepository;

impl UserRepository {
    pub async fn find_all(
        db: &impl ConnectionTrait,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<user::Model>, sea_orm::DbErr> {
        use sea_orm::QuerySelect;
        User::find().limit(limit).offset(offset).all(db).await
    }

    pub async fn find_by_id(
        db: &impl ConnectionTrait,
        id: Uuid,
    ) -> Result<Option<user::Model>, sea_orm::DbErr> {
        User::find_by_id(id).one(db).await
    }

    pub async fn find_by_email(
        db: &impl ConnectionTrait,
        email: &str,
    ) -> Result<Option<user::Model>, sea_orm::DbErr> {
        User::find()
            .filter(user::Column::Email.eq(email))
            .one(db)
            .await
    }

    pub async fn insert(
        db: &impl ConnectionTrait,
        model: user::ActiveModel,
    ) -> Result<user::Model, sea_orm::DbErr> {
        User::insert(model).exec_with_returning(db).await
    }

    /// Update a user model.
    ///
    /// Note: takes `&DatabaseConnection` rather than `&impl ConnectionTrait` because
    /// `ActiveModelTrait::update` in SeaORM requires a concrete `DatabaseConnection`.
    /// This means `update` cannot be called inside a transaction via `ConnectionTrait`.
    /// Use `update_many` with explicit filters for transaction-safe partial updates.
    pub async fn update(
        db: &DatabaseConnection,
        model: user::ActiveModel,
    ) -> Result<user::Model, sea_orm::DbErr> {
        let updated = model.update(db).await?;
        Ok(updated)
    }

    pub async fn delete(db: &impl ConnectionTrait, id: Uuid) -> Result<u64, sea_orm::DbErr> {
        let res = User::delete_by_id(id).exec(db).await?;
        Ok(res.rows_affected)
    }

    /// Atomically increment token_version for a user — single UPDATE, no prior SELECT.
    pub async fn increment_token_version(
        db: &impl ConnectionTrait,
        id: Uuid,
    ) -> Result<u64, sea_orm::DbErr> {
        use sea_orm::sea_query::Expr;
        let res = User::update_many()
            .col_expr(user::Column::TokenVersion, Expr::col(user::Column::TokenVersion).add(1))
            .filter(user::Column::Id.eq(id))
            .exec(db)
            .await?;
        Ok(res.rows_affected)
    }
}
