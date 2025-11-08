use sea_orm::prelude::DateTimeUtc;
use serde::Serialize;
use utoipa::ToSchema;

use crate::domains::typing_history;

#[derive(Serialize, ToSchema)]
pub struct TypingHistorySchema {
    pub id: i32,
    pub user_id: String,
    pub tournament_id: String,
    pub accuracy: i32,
    pub speed: i32,
    #[schema(value_type = String, format = DateTime)]
    pub completed_at: DateTimeUtc,
}

impl From<typing_history::Model> for TypingHistorySchema {
    fn from(session: typing_history::Model) -> Self {
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

#[derive(Serialize, ToSchema)]
pub struct TypingHistoryListSchema {
    pub typing_history: Vec<TypingHistorySchema>,
}

impl From<Vec<typing_history::Model>> for TypingHistoryListSchema {
    fn from(typing_history: Vec<typing_history::Model>) -> Self {
        Self {
            typing_history: typing_history
                .into_iter()
                .map(TypingHistorySchema::from)
                .collect(),
        }
    }
}
