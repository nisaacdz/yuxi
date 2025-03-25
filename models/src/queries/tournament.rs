use sea_orm::prelude::DateTime;
use serde::Deserialize;

#[derive(Deserialize, Default)]
pub struct TournamentQuery {
    pub title: String,
    pub scheduled_at: DateTime,
}
