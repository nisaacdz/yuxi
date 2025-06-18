use chrono::Utc;
use models::domains::sea_orm_active_enums::TournamentPrivacy;
use models::queries::TournamentPaginationQuery;
use models::schemas::pagination::PaginatedData;
use models::schemas::tournament::{Tournament, TournamentSchema};
use models::schemas::typing::TextOptions;
use models::schemas::user::UserSchema;
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
    client_id: &str,
) -> Result<Tournament, DbErr> {
    let created_by = users::Entity::find_by_id(tournament.created_by)
        .one(&app_state.conn)
        .await?
        .ok_or_else(|| DbErr::Custom("Tournament creator not found".into()))?;
    let manager = app_state.tournament_registry.get(&tournament.id);

    let live_data = if let Some(manager) = manager {
        Some(manager.live_data(client_id).await)
    } else {
        None
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
        creator: created_by.username,
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
    db: &DbConn,
    params: CreateTournamentParams,
    user: &UserSchema,
) -> Result<tournaments::Model, DbErr> {
    let id = nanoid::nanoid!(TOURNAMENT_ID_LENGTH, &super::ID_ALPHABET);

    tournaments::ActiveModel {
        id: Set(id),
        title: Set(params.title),
        description: Set(params.description),
        scheduled_for: Set(params.scheduled_for),
        created_by: Set(user.id.clone()),
        privacy: Set(TournamentPrivacy::Open),
        text_options: Set(params.text_options.map(TextOptions::to_value)),
        ..Default::default()
    }
    .insert(db)
    .await
}

pub async fn search_tournaments(
    app_state: &AppState,
    query: TournamentPaginationQuery,
    client_id: &str,
) -> Result<PaginatedData<Tournament>, DbErr> {
    let limit = query.limit.unwrap_or(15);
    let page = query.page.unwrap_or(1);
    let offset = (page - 1) * limit;

    let total = tournaments::Entity::find().count(&app_state.conn).await?;
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

        for m in sql_query.all(&app_state.conn).await?.into_iter() {
            res.push(parse_tournament(m, app_state, client_id).await?)
        }
        res
    };

    return Ok(PaginatedData::new(data, page, limit, total));
}

pub async fn get_tournament(db: &DbConn, id: String) -> Result<Option<TournamentSchema>, DbErr> {
    tournaments::Entity::find_by_id(id)
        .one(db)
        .await
        .map(|v| v.map(|v| v.into()))
}
