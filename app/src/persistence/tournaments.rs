use chrono::Utc;
use models::domains::sea_orm_active_enums::TournamentPrivacy;
use models::queries::TournamentPaginationQuery;
use models::schemas::pagination::PaginatedData;
use models::schemas::tournament::{Tournament, TournamentSchema};
use models::schemas::typing::TextOptions;
use models::schemas::user::{TournamentRoomMember, UserSchema};
use sea_orm::DbErr;

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

    let total = app_state
        .tables
        .tournaments
        .values()
        .into_iter()
        .filter(|t| {
            (if let Some(status) = query.status {
                match status {
                    models::schemas::typing::TournamentStatus::Upcoming => {
                        t.started_at.is_none() && t.ended_at.is_none()
                    }
                    models::schemas::typing::TournamentStatus::Started => {
                        t.started_at.is_some() && t.ended_at.is_none()
                    }
                    models::schemas::typing::TournamentStatus::Ended => t.ended_at.is_some(),
                }
            } else {
                true
            }) && (if let Some(privacy) = query.privacy {
                t.privacy == privacy
            } else {
                true
            })
        })
        .count() as u64;

    let data = {
        let mut res = Vec::new();

        for m in app_state
            .tables
            .tournaments
            .values()
            .into_iter()
            .filter(|t| {
                (if let Some(status) = query.status {
                    match status {
                        models::schemas::typing::TournamentStatus::Upcoming => {
                            t.started_at.is_none() && t.ended_at.is_none()
                        }
                        models::schemas::typing::TournamentStatus::Started => {
                            t.started_at.is_some() && t.ended_at.is_none()
                        }
                        models::schemas::typing::TournamentStatus::Ended => t.ended_at.is_some(),
                    }
                } else {
                    true
                }) && (if let Some(privacy) = query.privacy {
                    t.privacy == privacy
                } else {
                    true
                })
            })
            .skip(offset as usize)
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

pub async fn update_tournament(
    state: &AppState,
    params: UpdateTournamentParams,
) -> Result<tournaments::Model, DbErr> {
    let id = if let Some(id) = params.id {
        id
    } else {
        return Err(DbErr::AttrNotSet("tournament id".into()));
    };
    let v = state.tables.tournaments.update_data(&id, move |t| {
        if let Some(title) = params.title {
            t.title = title;
        }
        if let Some(description) = params.description {
            t.description = description;
        }
        if let Some(scheduled_for) = params.scheduled_for {
            t.scheduled_for = scheduled_for;
        }
        if let Some(text_options) = params.text_options {
            t.text_options = text_options.map(TextOptions::to_value);
        }
        if let Some(ended_at) = params.ended_at {
            t.ended_at = ended_at;
        }
        if let Some(started_at) = params.started_at {
            t.started_at = started_at;
        }

        t.clone()
    });

    v.ok_or(DbErr::RecordNotFound("Tournament not found".into()))
}
