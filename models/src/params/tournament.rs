use chrono::{DateTime, FixedOffset};
use serde::Deserialize;
use validator::Validate;

use crate::schemas::typing::TextOptions;

#[derive(Deserialize, Validate, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateTournamentParams {
    pub title: String,
    pub description: String,
    pub scheduled_for: DateTime<FixedOffset>,
    pub text_options: Option<TextOptions>,
}

#[derive(Deserialize, Validate, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTournamentParams {
    pub id: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub scheduled_for: Option<DateTime<FixedOffset>>,
    pub text_options: Option<Option<TextOptions>>,
    pub ended_at: Option<Option<DateTime<FixedOffset>>>,
}

impl Default for UpdateTournamentParams {
    fn default() -> Self {
        Self {
            id: None,
            title: None,
            description: None,
            text_options: None,
            scheduled_for: None,
            ended_at: None,
        }
    }
}
