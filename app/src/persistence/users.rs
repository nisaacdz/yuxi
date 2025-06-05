use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, EntityTrait, IntoActiveModel, QueryFilter, Set,
};

use models::domains::users;
use models::params::user::{
    CreateUserParams, LoginUserParams, ResetPasswordBody, UpdateUserParams,
};
use models::queries::user::UserQuery;

pub async fn create_user(
    db: &DbConn,
    params: CreateUserParams,
) -> Result<users::ActiveModel, DbErr> {
    let pass_hash = bcrypt::hash(params.password, 4).unwrap();

    let existing_user = users::Entity::find()
        .filter(users::Column::Email.eq(&params.email))
        .one(db)
        .await?;
    if existing_user.is_some() {
        return Err(DbErr::Custom("User already exists".to_string()));
    }

    users::ActiveModel {
        email: Set(params.email),
        passhash: Set(pass_hash),
        ..Default::default()
    }
    .save(db)
    .await
}

pub async fn update_user(
    db: &DbConn,
    id: i32,
    params: UpdateUserParams,
) -> Result<users::Model, DbErr> {
    let mut update_query = users::Entity::update_many().filter(users::Column::Id.eq(&id));

    if let Some(username) = params.username {
        update_query = update_query.col_expr(
            users::Column::Username,
            sea_orm::sea_query::Expr::value(username),
        );
    }

    update_query.exec(db).await?;

    users::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or(DbErr::RecordNotFound(
            "User not found after update".to_string(),
        ))
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
    let user = match users::Entity::find()
        .filter(users::Column::Email.eq(&email))
        .one(db)
        .await?
    {
        None => return Err(DbErr::RecordNotFound("User not found".to_string())),
        Some(user) => user,
    };
    if !bcrypt::verify(password, &user.passhash).unwrap() {
        return Err(DbErr::Custom("Password incorrect".to_string()));
    }
    Ok(user)
}

pub async fn reset_password(db: &DbConn, params: ResetPasswordBody) -> Result<String, DbErr> {
    use chrono::Utc;
    use models::domains::otp;
    use sea_orm::TransactionTrait;

    let txn = db.begin().await?;

    // 1. Find OTP record for the email
    let otp_record = otp::Entity::find_by_id(params.email.clone())
        .one(&txn)
        .await?;
    let otp_record = match otp_record {
        Some(r) => r,
        None => {
            txn.rollback().await?;
            return Err(DbErr::Custom("OTP not found".to_string()));
        }
    };

    // 2. Check OTP matches and is not expired (10 min window)
    let now = Utc::now();
    if otp_record.otp.to_string() != params.otp {
        txn.rollback().await?;
        return Err(DbErr::Custom("OTP incorrect".to_string()));
    }
    if now
        .signed_duration_since(otp_record.created_at)
        .num_minutes()
        > 10
    {
        txn.rollback().await?;
        return Err(DbErr::Custom("OTP expired".to_string()));
    }

    // 3. Hash new password
    let pass_hash = bcrypt::hash(params.password, 4).map_err(|e| DbErr::Custom(e.to_string()))?;

    // 4. Update user's password
    let user = users::Entity::find()
        .filter(users::Column::Email.eq(params.email.clone()))
        .one(&txn)
        .await?;
    let mut user = match user {
        Some(u) => u.into_active_model(),
        None => {
            txn.rollback().await?;
            return Err(DbErr::Custom("User not found".to_string()));
        }
    };
    user.passhash = Set(pass_hash);
    user.update(&txn).await?;

    // 5. Delete OTP
    otp::Entity::delete_by_id(params.email.clone())
        .exec(&txn)
        .await?;

    // 6. Commit transaction
    txn.commit().await?;
    Ok("Password reset successful".into())
}
