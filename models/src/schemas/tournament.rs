use chrono::{DateTime, Utc};
use sea_orm::prelude::DateTimeUtc;
use serde::{Deserialize, Serialize};

use crate::domains::{sea_orm_active_enums::TournamentPrivacy, tournaments};

use super::{text::TextOptions, user::UserSchema};

#[derive(Serialize, Clone, Debug)]
pub struct TournamentSchema {
    pub id: String,
    pub title: String,
    pub created_at: DateTimeUtc,
    pub created_by: i32,
    pub scheduled_for: DateTimeUtc,
    pub joined: i32,
    pub privacy: TournamentPrivacy,
    pub text_options: Option<TextOptions>,
    pub text_id: Option<i32>,
}

impl From<tournaments::Model> for TournamentSchema {
    fn from(tournament: tournaments::Model) -> Self {
        Self {
            id: tournament.id,
            title: tournament.title,
            created_at: tournament.created_at.to_utc(),
            created_by: tournament.created_by,
            scheduled_for: tournament.scheduled_for.to_utc(),
            joined: tournament.joined,
            privacy: tournament.privacy,
            text_options: tournament.text_options.map(TextOptions::from_value),
            text_id: tournament.text_id,
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
    pub text: Vec<char>,
    pub joined: i32,
    pub current: i32,
}

impl TournamentSession {
    pub fn new(id: String, scheduled_for: DateTime<Utc>, text: Vec<char>) -> Self {
        Self {
            id,
            scheduled_for,
            started_at: None,
            ended_at: None,
            text,
            joined: 0,
            current: 0,
        }
    }
}

#[derive(Serialize)]
pub struct TournamentUpcomingSchema {
    pub id: String,
    pub title: String,
    pub created_at: DateTimeUtc,
    pub created_by: UserSchema,
    pub scheduled_for: DateTimeUtc,
    pub joined: i32,
    pub privacy: TournamentPrivacy,
    pub text_options: Option<TextOptions>,
}
