use chrono::{DateTime, Utc};
use sea_orm::prelude::DateTimeUtc;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::domains::{sea_orm_active_enums::TournamentPrivacy, tournaments};

use super::typing::TextOptions;

#[derive(Serialize, Clone, Debug, ToSchema)]
pub struct TournamentSchema {
    pub id: String,
    pub title: String,
    pub description: String,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: DateTimeUtc,
    pub created_by: String,
    #[schema(value_type = String, format = DateTime)]
    pub scheduled_for: DateTimeUtc,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub started_at: Option<DateTimeUtc>,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub ended_at: Option<DateTimeUtc>,
    pub privacy: TournamentPrivacy,
    pub text_options: Option<TextOptions>,
}

impl From<tournaments::Model> for TournamentSchema {
    fn from(tournament: tournaments::Model) -> Self {
        Self {
            id: tournament.id,
            title: tournament.title,
            description: tournament.description,
            created_at: tournament.created_at.to_utc(),
            created_by: tournament.created_by,
            scheduled_for: tournament.scheduled_for.to_utc(),
            started_at: tournament.started_at.map(|v| v.to_utc()),
            ended_at: tournament.ended_at.map(|v| v.to_utc()),
            privacy: tournament.privacy,
            text_options: tournament.text_options.map(TextOptions::from_value),
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct TournamentListSchema {
    pub tournaments: Vec<TournamentSchema>,
}

impl From<Vec<tournaments::Model>> for TournamentListSchema {
    fn from(tournaments: Vec<tournaments::Model>) -> Self {
        Self {
            tournaments: tournaments
                .into_iter()
                .map(TournamentSchema::from)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct TournamentSession {
    pub id: String,
    pub scheduled_for: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub text: Option<String>,
    pub scheduled_end: Option<DateTime<Utc>>,
}

impl TournamentSession {
    pub fn new(id: String, scheduled_for: DateTime<Utc>, text: Option<String>) -> Self {
        Self {
            id,
            scheduled_for,
            started_at: None,
            ended_at: None,
            text,
            scheduled_end: None,
        }
    }
}

#[derive(Serialize, Debug, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Tournament {
    pub id: String,
    pub title: String,
    pub creator: String,
    #[schema(value_type = String, format = DateTime)]
    pub scheduled_for: DateTimeUtc,
    pub description: String,
    pub privacy: TournamentPrivacy,
    pub text_options: Option<TextOptions>,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub started_at: Option<DateTimeUtc>,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub ended_at: Option<DateTimeUtc>,
    pub participating: bool,
    pub participant_count: usize,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TournamentLiveData {
    pub participant_count: usize,
    pub participating: bool,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub started_at: Option<DateTimeUtc>,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub ended_at: Option<DateTimeUtc>,
}
