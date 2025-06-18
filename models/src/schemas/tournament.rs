use chrono::{DateTime, Utc};
use sea_orm::prelude::DateTimeUtc;
use serde::{Deserialize, Serialize};

use crate::domains::{sea_orm_active_enums::TournamentPrivacy, tournaments};

use super::typing::TextOptions;

#[derive(Serialize, Clone, Debug)]
pub struct TournamentSchema {
    pub id: String,
    pub title: String,
    pub description: String,
    pub created_at: DateTimeUtc,
    pub created_by: String,
    pub scheduled_for: DateTimeUtc,
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
            privacy: tournament.privacy,
            text_options: tournament.text_options.map(TextOptions::from_value),
        }
    }
}

#[derive(Serialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TournamentSession {
    pub id: String,
    pub scheduled_for: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub text: Option<String>,
    pub current: i32,
}

impl TournamentSession {
    pub fn new(id: String, scheduled_for: DateTime<Utc>, text: Option<String>) -> Self {
        Self {
            id,
            scheduled_for,
            started_at: None,
            ended_at: None,
            text,
            current: 0,
        }
    }
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Tournament {
    pub id: String,
    pub title: String,
    pub creator: String,
    pub scheduled_for: DateTimeUtc,
    pub description: String,
    pub privacy: TournamentPrivacy,
    pub text_options: Option<TextOptions>,
    pub started_at: Option<DateTimeUtc>,
    pub ended_at: Option<DateTimeUtc>,
    pub participating: bool,
    pub participant_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TournamentLiveData {
    pub participant_count: usize,
    pub participating: bool,
    pub started_at: Option<DateTimeUtc>,
    pub ended_at: Option<DateTimeUtc>,
}
