use sea_orm::prelude::DateTime;
use serde::Deserialize;
use validator::Validate;

#[derive(Deserialize, Validate)]
pub struct CreateTournamentParams {
    pub title: String,
    pub scheduled_for: DateTime,
}
