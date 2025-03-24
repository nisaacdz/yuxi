use sea_orm::prelude::DateTime;
use serde::Deserialize;
use utoipa::IntoParams;

#[derive(Deserialize, Default, IntoParams)]
#[into_params(style = Form, parameter_in = Query)]
pub struct TournamentQuery {
    #[param(nullable = true)]
    pub title: String,
    pub scheduled_at: DateTime,
}
