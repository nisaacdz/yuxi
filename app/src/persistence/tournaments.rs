use chrono::Utc;
use models::domains::sea_orm_active_enums::TournamentPrivacy;
use models::queries::TournamentPaginationQuery;
use models::schemas::pagination::PaginatedData;
use models::schemas::tournament::{Tournament, TournamentSchema};
use models::schemas::typing::TextOptions;
use models::schemas::user::{TournamentRoomMember, UserSchema};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};

use models::domains::*;
use models::params::tournament::CreateTournamentParams;

use crate::state::AppState;

const TOURNAMENT_ID_LENGTH: usize = 24;

pub async fn parse_tournament(
    tournament: tournaments::Model,
    app_state: &AppState,
    user_id: Option<&str>,
    member_id: Option<&str>,
) -> Result<Tournament, DbErr> {
    let creator = app_state
        .tables
        .users
        .get_data(&tournament.created_by)
        .map(|u| u.username)
        .unwrap_or("Anonymous".into());
    let manager = app_state.tournament_registry.get(&tournament.id);

    let live_data = match (manager, user_id, member_id) {
        (Some(manager), Some(user_id), _) => Some(
            manager
                .live_data(&TournamentRoomMember::get_id(user_id))
                .await,
        ),
        (Some(manager), _, Some(member_id)) => Some(manager.live_data(member_id).await),
        _ => None,
    };

    let mut started_at = tournament.started_at.map(|v| v.to_utc());
    let mut participant_count = 0;
    let mut participating = false;
    let mut ended_at = tournament.ended_at.map(|v| v.to_utc());

    if let Some(live_data) = live_data {
        started_at = live_data.started_at;
        participant_count = live_data.participant_count;
        participating = live_data.participating;
        ended_at = live_data.ended_at;
    }

    Ok(Tournament {
        id: tournament.id,
        title: tournament.title,
        creator,
        description: tournament.description,
        started_at,
        ended_at,
        participating,
        participant_count,
        scheduled_for: tournament.scheduled_for.to_utc(),
        privacy: tournament.privacy,
        text_options: tournament.text_options.map(TextOptions::from_value),
    })
}

pub async fn create_tournament(
    state: &AppState,
    params: CreateTournamentParams,
    user: &UserSchema,
) -> Result<tournaments::Model, DbErr> {
    let id = nanoid::nanoid!(TOURNAMENT_ID_LENGTH, &super::ID_ALPHABET);

    let new_tournament = tournaments::Model {
        id: id.clone(),
        title: params.title,
        description: params.description,
        scheduled_for: params.scheduled_for,
        created_by: user.id.clone(),
        privacy: TournamentPrivacy::Open,
        text_options: params.text_options.map(TextOptions::to_value),
        created_at: Utc::now().fixed_offset(),
        updated_at: Utc::now().fixed_offset(),
        started_at: None,
        ended_at: None,
    };

    state
        .tables
        .tournaments
        .set_data(&id, new_tournament.clone());

    Ok(new_tournament)
}

pub async fn search_tournaments(
    app_state: &AppState,
    query: TournamentPaginationQuery,
    user_id: Option<&str>,
    member_id: Option<&str>,
) -> Result<PaginatedData<Tournament>, DbErr> {
    let limit = query.limit.unwrap_or(15);
    let page = query.page.unwrap_or(1);
    let offset = (page - 1) * limit;

    let total = app_state.tables.tournaments.count() as u64;
    let data = {
        let mut res = Vec::new();

        let mut sql_query = tournaments::Entity::find();

        if let Some(privacy) = query.privacy {
            sql_query = sql_query.filter(tournaments::Column::Privacy.eq(privacy))
        }

        if let Some(status) = query.status {
            match status {
                models::schemas::typing::TournamentStatus::Upcoming => {
                    sql_query = sql_query.filter(tournaments::Column::ScheduledFor.gt(Utc::now()))
                }
                models::schemas::typing::TournamentStatus::Started => {
                    sql_query = sql_query.filter(
                        tournaments::Column::StartedAt
                            .is_not_null()
                            .and(tournaments::Column::EndedAt.is_null()),
                    )
                }
                models::schemas::typing::TournamentStatus::Ended => {
                    sql_query = sql_query.filter(tournaments::Column::EndedAt.is_not_null())
                }
            }
        }

        if let Some(search) = query.search {
            if !search.is_empty() {
                sql_query = sql_query.filter(
                    tournaments::Column::Title
                        .like(&search)
                        .or(tournaments::Column::Description.like(&search))
                        .or(users::Column::Username.like(&search)),
                );
            }
        }

        let sql_query = sql_query
            .order_by_asc(tournaments::Column::ScheduledFor)
            .offset(offset)
            .limit(limit);

        for m in app_state
            .tables
            .tournaments
            .values()
            .into_iter()
            .take(limit as usize)
        {
            res.push(parse_tournament(m, app_state, user_id, member_id).await?)
        }
        res
    };

    return Ok(PaginatedData::new(data, page, limit, total));
}

pub async fn get_tournament(
    state: &AppState,
    id: String,
) -> Result<Option<TournamentSchema>, DbErr> {
    Ok(state.tables.tournaments.get_data(&id).map(From::from))
}
