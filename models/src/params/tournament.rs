use sea_orm::prelude::DateTime;
use serde::Deserialize;
use utoipa::ToSchema;
use validator::Validate;

#[derive(Deserialize, Validate, ToSchema)]
pub struct CreateTournamentParams {
    pub title: String,
    pub scheduled_for: DateTime,
}
