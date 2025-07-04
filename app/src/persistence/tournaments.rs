use chrono::Utc;
use models::domains::sea_orm_active_enums::TournamentPrivacy;
use models::queries::TournamentPaginationQuery;
use models::schemas::pagination::PaginatedData;
use models::schemas::tournament::{Tournament, TournamentSchema};
use models::schemas::typing::TextOptions;
use models::schemas::user::{TournamentRoomMember, UserSchema};
use sea_orm::ActiveValue::Unchanged;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};

use models::domains::*;
use models::params::tournament::{CreateTournamentParams, UpdateTournamentParams};

use crate::state::AppState;

const TOURNAMENT_ID_LENGTH: usize = 24;

pub async fn parse_tournament(
    tournament: tournaments::Model,
    app_state: &AppState,
    user_id: Option<&str>,
    member_id: Option<&str>,
) -> Result<Tournament, DbErr> {
    let created_by = users::Entity::find_by_id(tournament.created_by)
        .one(&app_state.conn)
        .await?
        .ok_or_else(|| DbErr::Custom("Tournament creator not found".into()))?;
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

pub async fn update_tournament(
    state: &AppState,
    params: UpdateTournamentParams,
) -> Result<tournaments::Model, DbErr> {
    let id = if let Some(id) = params.id {
        id
    } else {
        return Err(DbErr::AttrNotSet("id".into()));
    };
    let mut tournament: tournaments::ActiveModel = tournaments::ActiveModel {
        id: Unchanged(id),
        ..Default::default()
    };

    if let Some(title) = params.title {
        tournament.title = Set(title);
    }

    if let Some(description) = params.description {
        tournament.description = Set(description);
    }

    if let Some(scheduled_for) = params.scheduled_for {
        tournament.scheduled_for = Set(scheduled_for);
    }

    if let Some(ended_at) = params.ended_at {
        tournament.ended_at = Set(ended_at);
    }

    if let Some(text_options) = params.text_options {
        tournament.text_options = Set(text_options.map(TextOptions::to_value));
    }

    tournament.update(&state.conn).await
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
            res.push(parse_tournament(m, app_state, user_id, member_id).await?)
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
