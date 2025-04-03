use chrono::{DateTime, FixedOffset};
use serde::Deserialize;
use validator::Validate;

#[derive(Deserialize, Validate)]
pub struct CreateTournamentParams {
    pub title: String,
    pub scheduled_for: DateTime<FixedOffset>,
}
