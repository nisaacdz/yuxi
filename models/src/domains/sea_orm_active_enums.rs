use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "tournament_privacy")]
pub enum TournamentPrivacy {
    #[sea_orm(string_value = "open")]
    Open,
    #[sea_orm(string_value = "invitational")]
    Invitational,
}
