//
// SeaORM entity definition (users table) with production-grade auth fields.
//
// Notes:
// - `password_hash` stores a secure Argon2 (or similar) hash; do NOT store plaintext.
// - `token_version` is used to invalidate issued JWTs when you want to revoke sessions
//   (increment the token_version on password change / global logout).
// - `is_active` allows disabling accounts without deleting them.
// - `last_login` is optional and can be updated on successful auth.
//
// When adding these fields, also add appropriate DB migration changes (unique index on email,
// non-null constraints, defaults for is_active and token_version, and an index if needed).
//
pub mod user {
    use crate::refresh_tokens::refresh_token::Entity as RefreshTokens;
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Eq)]
    #[sea_orm(table_name = "users")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub name: String,
        #[sea_orm(unique)]
        pub email: String,
        #[sea_orm(column_type = "Text")]
        pub password_hash: String,
        pub is_active: bool,
        pub last_login: Option<DateTimeWithTimeZone>,
        pub created_at: Option<DateTimeWithTimeZone>,
        pub updated_at: Option<DateTimeWithTimeZone>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(has_many = "RefreshTokens")]
        RefreshTokens,
    }

    impl Related<RefreshTokens> for Entity {
        fn to() -> RelationDef {
            Relation::RefreshTokens.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

pub use user::Entity as User;
