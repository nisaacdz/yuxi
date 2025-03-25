//! `SeaORM` Entity, @generated by sea-orm-codegen 1.1.7

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(schema_name = "public", table_name = "tournaments")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub title: String,
    pub created_at: DateTime,
    pub created_by: i32,
    pub scheduled_for: DateTime,
    pub started_at: Option<DateTime>,
    pub privacy: Option<String>,
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub text_options: Option<Json>,
    pub text_id: Option<i32>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::sessions::Entity")]
    Sessions,
    #[sea_orm(
        belongs_to = "super::texts::Entity",
        from = "Column::TextId",
        to = "super::texts::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Texts,
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::CreatedBy",
        to = "super::users::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Users,
}

impl Related<super::sessions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Sessions.def()
    }
}

impl Related<super::texts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Texts.def()
    }
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Users.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
