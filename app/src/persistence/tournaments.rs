use models::queries::tournament::TournamentQuery;
use sea_orm::{ActiveModelTrait, DbConn, DbErr, EntityTrait, Set};

use models::domains::tournaments;
use models::params::tournament::CreateTournamentParams;

pub async fn create_tournament(
    db: &DbConn,
    params: CreateTournamentParams,
) -> Result<tournaments::ActiveModel, DbErr> {
    tournaments::ActiveModel {
        title: Set(params.title),
        ..Default::default()
    }
    .save(db)
    .await
}

pub async fn search_tournaments(
    db: &DbConn,
    _query: TournamentQuery,
) -> Result<Vec<tournaments::Model>, DbErr> {
    tournaments::Entity::find().all(db).await
}

pub async fn get_tournament(db: &DbConn, id: String) -> Result<Option<tournaments::Model>, DbErr> {
    tournaments::Entity::find_by_id(id).one(db).await
}
