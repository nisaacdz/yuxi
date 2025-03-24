use sea_orm::{ActiveModelTrait, ColumnTrait, DbConn, DbErr, EntityTrait, QueryFilter, Set};

use models::domains::users;
use models::params::user::CreateUserParams;
use models::queries::user::UserQuery;

pub async fn create_user(
    db: &DbConn,
    params: CreateUserParams,
) -> Result<users::ActiveModel, DbErr> {
    users::ActiveModel {
        username: Set(params.username),
        ..Default::default()
    }
    .save(db)
    .await
}

pub async fn search_users(db: &DbConn, query: UserQuery) -> Result<Vec<users::Model>, DbErr> {
    users::Entity::find()
        .filter(users::Column::Username.contains(query.username))
        .all(db)
        .await
}

pub async fn get_user(db: &DbConn, id: i32) -> Result<Option<users::Model>, DbErr> {
    users::Entity::find_by_id(id).one(db).await
}
