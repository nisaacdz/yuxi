use sea_orm::prelude::{DateTimeUtc, Decimal};
use serde::Serialize;

use crate::domains::sessions;

#[derive(Serialize)]
pub struct CompletedSessionSchema {
    pub id: i32,
    pub user_id: i32,
    pub tournament_id: String,
    pub text_id: i32,
    pub accuracy: Option<Decimal>,
    pub speed: Option<Decimal>,
    pub created_at: Option<DateTimeUtc>,
}

impl From<sessions::Model> for CompletedSessionSchema {
    fn from(session: sessions::Model) -> Self {
        Self {
            id: session.id,
            user_id: session.user_id,
            tournament_id: session.tournament_id,
            text_id: session.text_id,
            accuracy: session.accuracy,
            speed: session.speed,
            created_at: session.created_at,
        }
    }
}

#[derive(Serialize)]
pub struct CompletedSessionListSchema {
    pub sessions: Vec<CompletedSessionSchema>,
}

impl From<Vec<sessions::Model>> for CompletedSessionListSchema {
    fn from(sessions: Vec<sessions::Model>) -> Self {
        Self {
            sessions: sessions
                .into_iter()
                .map(CompletedSessionSchema::from)
                .collect(),
        }
    }
}
