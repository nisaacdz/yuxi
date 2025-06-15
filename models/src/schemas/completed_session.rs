use sea_orm::prelude::DateTimeUtc;
use serde::Serialize;

use crate::domains::completed_sessions;

#[derive(Serialize)]
pub struct CompletedSessionSchema {
    pub id: i32,
    pub user_id: i32,
    pub tournament_id: String,
    pub accuracy: i32,
    pub speed: i32,
    pub completed_at: DateTimeUtc,
}

impl From<completed_sessions::Model> for CompletedSessionSchema {
    fn from(session: completed_sessions::Model) -> Self {
        Self {
            id: session.id,
            user_id: session.user_id,
            tournament_id: session.tournament_id,
            accuracy: session.accuracy,
            speed: session.speed,
            completed_at: session.completed_at.to_utc(),
        }
    }
}

#[derive(Serialize)]
pub struct CompletedSessionListSchema {
    pub completed_sessions: Vec<CompletedSessionSchema>,
}

impl From<Vec<completed_sessions::Model>> for CompletedSessionListSchema {
    fn from(completed_sessions: Vec<completed_sessions::Model>) -> Self {
        Self {
            completed_sessions: completed_sessions
                .into_iter()
                .map(CompletedSessionSchema::from)
                .collect(),
        }
    }
}
