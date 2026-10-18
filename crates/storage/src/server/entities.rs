pub(super) mod admin {
    use sea_orm::entity::prelude::*;
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "server_admin")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub singleton: i64,
        pub username: String,
        pub password_hash: String,
        pub auth_version: String,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}
pub(super) mod session {
    use sea_orm::entity::prelude::*;
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "server_sessions")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id_hash: String,
        pub payload: String,
        pub expires_at: i64,
        pub auth_version: String,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}
pub(super) mod device {
    use sea_orm::entity::prelude::*;
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "server_devices")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: String,
        pub name: String,
        pub token_hash: String,
        pub created_at: i64,
        pub revoked_at: Option<i64>,
        pub last_seen_at: Option<i64>,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}
