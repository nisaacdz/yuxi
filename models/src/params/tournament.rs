use sea_orm::sqlx::types::chrono::{DateTime, Utc};
use serde::Deserialize;
use validator::Validate;

#[derive(Deserialize, Validate)]
pub struct CreateTournamentParams {
    pub title: String,
    pub scheduled_for: DateTime<Utc>,
}
