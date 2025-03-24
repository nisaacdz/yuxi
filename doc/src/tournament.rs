use utoipa::OpenApi;

use models::params::tournament::CreateTournamentParams;
use models::schemas::tournament::{TournamentListSchema, TournamentSchema};

use api::models::{ApiErrorResponse, ParamsErrorResponse};
use api::routers::tournament::*;

#[derive(OpenApi)]
#[openapi(
    paths(tournaments_get, tournaments_post),
    components(schemas(
        CreateTournamentParams,
        TournamentListSchema,
        TournamentSchema,
        ApiErrorResponse,
        ParamsErrorResponse,
    ))
)]
pub(super) struct TournamentApi;
