use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    #[sea_orm(unique)]
    pub username: String,
    #[sea_orm(unique)]
    pub email: String,
    pub passhash: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::typing_history::Entity")]
    TypingHistorys,
    #[sea_orm(has_many = "super::tournaments::Entity")]
    Tournaments,
}

impl Related<super::typing_history::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TypingHistorys.def()
    }
}

impl Related<super::tournaments::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tournaments.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
