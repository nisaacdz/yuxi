//! `SeaORM` Entity, @generated by sea-orm-codegen 1.1.7

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "texts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(column_type = "Text")]
    pub content: String,
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub options: Option<Json>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::completed_sessions::Entity")]
    CompletedSessions,
    #[sea_orm(has_many = "super::tournaments::Entity")]
    Tournaments,
}

impl Related<super::completed_sessions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CompletedSessions.def()
    }
}

impl Related<super::tournaments::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tournaments.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
