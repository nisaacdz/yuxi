use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

use crate::{domains::sea_orm_active_enums::TournamentPrivacy, schemas::typing::TournamentStatus};

pub mod user;

#[derive(Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
pub struct TournamentPaginationQuery {
    pub page: Option<u64>,
    pub limit: Option<u64>,
    pub privacy: Option<TournamentPrivacy>,
    pub status: Option<TournamentStatus>,
    pub search: Option<String>,
}

impl Default for TournamentPaginationQuery {
    fn default() -> Self {
        Self {
            page: Some(1),
            limit: Some(15),
            privacy: None,
            status: None,
            search: None,
        }
    }
}
