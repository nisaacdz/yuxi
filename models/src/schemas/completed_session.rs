use sea_orm::prelude::{DateTime, Decimal};
use serde::Serialize;
use utoipa::ToSchema;

use crate::domains::completed_sessions;

#[derive(Serialize, ToSchema)]
pub struct CompletedSessionSchema {
    pub id: i32,
    pub user_id: i32,
    pub tournament_id: String,
    pub text_id: i32,
    pub accuracy: Option<Decimal>,
    pub wpm: Option<Decimal>,
    pub created_at: Option<DateTime>,
}

impl From<completed_sessions::Model> for CompletedSessionSchema {
    fn from(session: completed_sessions::Model) -> Self {
        Self {
            id: session.id,
            user_id: session.user_id,
            tournament_id: session.tournament_id,
            text_id: session.text_id,
            accuracy: session.accuracy,
            wpm: session.wpm,
            created_at: session.created_at,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct CompletedSessionListSchema {
    pub sessions: Vec<CompletedSessionSchema>,
}

impl From<Vec<completed_sessions::Model>> for CompletedSessionListSchema {
    fn from(sessions: Vec<completed_sessions::Model>) -> Self {
        Self {
            sessions: sessions
                .into_iter()
                .map(CompletedSessionSchema::from)
                .collect(),
        }
    }
}
