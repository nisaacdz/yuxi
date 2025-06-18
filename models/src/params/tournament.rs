use chrono::{DateTime, FixedOffset};
use serde::Deserialize;
use validator::Validate;

use crate::schemas::typing::TextOptions;

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateTournamentParams {
    pub title: String,
    pub description: String,
    pub scheduled_for: DateTime<FixedOffset>,
    pub text_options: Option<TextOptions>,
}
