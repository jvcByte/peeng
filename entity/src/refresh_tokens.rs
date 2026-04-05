//
// SeaORM entity definition (refresh_tokens table)
//
pub mod refresh_token {
    use crate::users::{User, user};
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "refresh_tokens")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub user_id: Uuid,
        pub token: String,
        pub token_version: i32,
        pub revoked: bool,
        pub expires_at: Option<DateTimeWithTimeZone>,
        pub created_at: Option<DateTimeWithTimeZone>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "User",
            from = "Column::UserId",
            to = "user::Column::Id",
            on_update = "NoAction",
            on_delete = "Cascade"
        )]
        Users,
    }

    impl Related<User> for Entity {
        fn to() -> RelationDef {
            Relation::Users.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

pub use refresh_token::Entity as RefreshToken;
