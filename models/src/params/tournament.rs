use chrono::{DateTime, FixedOffset};
use serde::Deserialize;
use utoipa::ToSchema;
use validator::Validate;

use crate::schemas::typing::TextOptions;

#[derive(Deserialize, Validate, Debug, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateTournamentParams {
    pub title: String,
    pub description: String,
    pub scheduled_for: DateTime<FixedOffset>,
    pub text_options: Option<TextOptions>,
}

#[derive(Deserialize, Validate, Debug, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTournamentParams {
    pub id: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub scheduled_for: Option<DateTime<FixedOffset>>,
    pub text_options: Option<Option<TextOptions>>,
    pub ended_at: Option<Option<DateTime<FixedOffset>>>,
}
