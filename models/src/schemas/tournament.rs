use chrono::{DateTime, Utc};
use sea_orm::prelude::DateTimeUtc;
use serde::{Deserialize, Serialize};

use crate::domains::tournaments;

#[derive(Serialize)]
pub struct TournamentSchema {
    pub id: String,
    pub title: String,
    pub created_at: DateTimeUtc,
    pub created_by: i32,
    pub scheduled_for: DateTimeUtc,
}

impl From<tournaments::Model> for TournamentSchema {
    fn from(tournament: tournaments::Model) -> Self {
        Self {
            id: tournament.id,
            title: tournament.title,
            created_at: tournament.created_at,
            created_by: tournament.created_by,
            scheduled_for: tournament.scheduled_for,
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
pub struct TournamentInfo {
    pub id: String,
    pub scheduled_for: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub text: Vec<char>,
    pub total_joined: i32,
    pub total_remaining: i32,
    pub total_completed: i32,
    pub automatized: bool,
}

impl TournamentInfo {
    pub fn new(id: String, scheduled_for: DateTime<Utc>, text: Vec<char>) -> Self {
        Self {
            id,
            scheduled_for,
            started_at: None,
            ended_at: None,
            text,
            total_joined: 0,
            total_remaining: 0,
            total_completed: 0,
            automatized: false,
        }
    }
}
