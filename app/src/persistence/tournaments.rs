use models::queries::PaginationQuery;
use models::schemas::pagination::PaginatedData;
use models::schemas::tournament::TournamentSchema;
use sea_orm::{ActiveModelTrait, DbConn, DbErr, EntityTrait, PaginatorTrait, QuerySelect, Set};

use models::domains::tournaments;
use models::params::tournament::CreateTournamentParams;

pub async fn create_tournament(
    db: &DbConn,
    params: CreateTournamentParams,
) -> Result<tournaments::ActiveModel, DbErr> {
    tournaments::ActiveModel {
        title: Set(params.title),
        scheduled_for: Set(params.scheduled_for),
        ..Default::default()
    }
    .save(db)
    .await
}

pub async fn search_tournaments(
    db: &DbConn,
    query: PaginationQuery,
) -> Result<PaginatedData<TournamentSchema>, DbErr> {
    let limit = query.limit.unwrap_or(15);
    let page = query.page.unwrap_or(1);
    let offset = (page - 1) * limit;

    let total = tournaments::Entity::find().count(db).await?;
    let data = tournaments::Entity::find()
        .offset(offset)
        .limit(limit)
        .all(db)
        .await?
        .into_iter()
        .map(TournamentSchema::from)
        .collect::<Vec<_>>();

    return Ok(PaginatedData::new(data, page, limit, total));
}

pub async fn get_tournament(db: &DbConn, id: String) -> Result<Option<TournamentSchema>, DbErr> {
    tournaments::Entity::find_by_id(id)
        .one(db)
        .await
        .map(|v| v.map(|v| v.into()))
}
