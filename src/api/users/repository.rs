use crate::shared::models::users::{User, user};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

pub struct UserRepository;

impl UserRepository {
    pub async fn find_all(db: &DatabaseConnection) -> Result<Vec<user::Model>, sea_orm::DbErr> {
        User::find().all(db).await
    }

    pub async fn find_by_id(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<Option<user::Model>, sea_orm::DbErr> {
        User::find_by_id(id).one(db).await
    }

    pub async fn find_by_email(
        db: &DatabaseConnection,
        email: &str,
    ) -> Result<Option<user::Model>, sea_orm::DbErr> {
        // Use a DB-level filter so the database can perform the lookup (and use an index).
        // This is more efficient and avoids loading all rows into memory.
        User::find()
            .filter(user::Column::Email.eq(email.to_owned()))
            .one(db)
            .await
    }

    pub async fn insert(
        db: &DatabaseConnection,
        model: user::ActiveModel,
    ) -> Result<user::Model, sea_orm::DbErr> {
        let inserted = User::insert(model).exec_with_returning(db).await?;

        Ok(inserted)
    }

    pub async fn update(
        db: &DatabaseConnection,
        model: user::ActiveModel,
    ) -> Result<user::Model, sea_orm::DbErr> {
        let updated = model.update(db).await?;
        Ok(updated)
    }

    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<u64, sea_orm::DbErr> {
        let res = User::delete_by_id(id).exec(db).await?;
        Ok(res.rows_affected)
    }
}
