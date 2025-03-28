use sea_orm::{ActiveModelTrait, ColumnTrait, DbConn, DbErr, EntityTrait, QueryFilter, Set};

use models::domains::users;
use models::params::user::{CreateUserParams, LoginUserParams};
use models::queries::user::UserQuery;

pub async fn create_user(
    db: &DbConn,
    params: CreateUserParams,
) -> Result<users::ActiveModel, DbErr> {
    let pass_hash = bcrypt::hash(params.password, 4).unwrap();
    let email = params.email.clone();
    let existing_user = users::Entity::find()
        .filter(users::Column::Email.eq(email))
        .one(db)
        .await?;
    if existing_user.is_some() {
        return Err(DbErr::Custom("User already exists".to_string()));
    }
    let username = params.username.clone();
    let existing_username = users::Entity::find()
        .filter(users::Column::Username.eq(username))
        .one(db)
        .await?;
    if existing_username.is_some() {
        return Err(DbErr::Custom("Username already exists".to_string()));
    }
    users::ActiveModel {
        email: Set(params.email),
        passhash: Set(pass_hash),
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

pub async fn login_user(
    db: &DbConn,
    LoginUserParams { email, password }: LoginUserParams,
) -> Result<users::Model, DbErr> {
    let pass_hash = bcrypt::hash(password, 4).unwrap();
    users::Entity::find()
        .filter(users::Column::Email.eq(email))
        .filter(users::Column::Passhash.eq(pass_hash))
        .one(db)
        .await?
        .ok_or(DbErr::Custom("User not found".to_string()))
}
