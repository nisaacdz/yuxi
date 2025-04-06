use chrono::Utc;
use models::queries::PaginationQuery;
use models::schemas::pagination::PaginatedData;
use models::schemas::text::TextOptions;
use models::schemas::tournament::{TournamentSchema, TournamentUpcomingSchema};
use models::schemas::user::UserSchema;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, EntityTrait, PaginatorTrait, QueryFilter,
    QuerySelect, Set,
};

use models::domains::tournaments;
use models::params::tournament::CreateTournamentParams;

pub async fn parse_upcoming_tournament(
    tournament: tournaments::Model,
    conn: &DbConn,
) -> Result<TournamentUpcomingSchema, DbErr> {
    let created_by = models::domains::users::Entity::find_by_id(tournament.created_by)
        .one(conn)
        .await?
        .ok_or_else(|| DbErr::Custom("Tournament creator not found".into()))?;
    Ok(TournamentUpcomingSchema {
        id: tournament.id,
        title: tournament.title,
        created_at: tournament.created_at.to_utc(),
        created_by: models::schemas::user::UserSchema::from(created_by),
        scheduled_for: tournament.scheduled_for.to_utc(),
        joined: tournament.joined,
        privacy: tournament.privacy,
        text_options: tournament.text_options.map(TextOptions::from_value),
    })
}

pub async fn create_tournament(
    db: &DbConn,
    params: CreateTournamentParams,
    user: &UserSchema,
) -> Result<tournaments::ActiveModel, DbErr> {
    tournaments::ActiveModel {
        title: Set(params.title),
        scheduled_for: Set(params.scheduled_for),
        created_by: Set(user.id.clone()),
        ..Default::default()
    }
    .save(db)
    .await
}

pub async fn search_upcoming_tournaments(
    db: &DbConn,
    query: PaginationQuery,
) -> Result<PaginatedData<TournamentUpcomingSchema>, DbErr> {
    let limit = query.limit.unwrap_or(15);
    let page = query.page.unwrap_or(1);
    let offset = (page - 1) * limit;

    let total = tournaments::Entity::find().count(db).await?;
    let data = {
        let mut res = Vec::new();
        for m in tournaments::Entity::find()
            .filter(models::domains::tournaments::Column::ScheduledFor.gt(Utc::now()))
            .offset(offset)
            .limit(limit)
            .all(db)
            .await?
            .into_iter()
        {
            res.push(parse_upcoming_tournament(m, db).await?)
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
