use sea_orm::prelude::DateTime;
use serde::Serialize;

use crate::domains::tournaments;

#[derive(Serialize)]
pub struct TournamentSchema {
    pub id: String,
    pub title: String,
    pub created_at: DateTime,
    pub created_by: i32,
    pub scheduled_for: DateTime,
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
