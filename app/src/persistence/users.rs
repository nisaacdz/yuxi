use fake::{Fake, faker};
use rand::Rng;
use rand::{SeedableRng, rngs::StdRng};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, IntoActiveModel, QueryFilter, Set,
};

use models::domains::{otp, users};
use models::params::user::{
    CreateUserParams, EmailAuthParams, ForgotPasswordBody, LoginUserParams, ResetPasswordBody,
    UpdateUserParams,
};
use models::queries::user::UserQuery;

use chrono::{TimeDelta, Utc};

use crate::state::AppState;

const OTP_DURATION: TimeDelta = TimeDelta::minutes(10);

const USER_ID_LENGTH: usize = 12;

const USERNAME_SUFFIX_LENGTH: usize = 6;

pub async fn create_user(
    state: &AppState,
    params: CreateUserParams,
) -> Result<users::Model, DbErr> {
    let pass_hash = bcrypt::hash(params.password, 4).unwrap();

    let existing_user = users::Entity::find()
        .filter(users::Column::Email.eq(&params.email))
        .one(&state.conn)
        .await?;

    if existing_user.is_some() {
        return Err(DbErr::Custom("User already exists".to_string()));
    }

    let id = nanoid::nanoid!(USER_ID_LENGTH, &super::ID_ALPHABET);

    let username = format!(
        "{}{}",
        faker::internet::en::Username().fake::<String>(),
        nanoid::nanoid!(USERNAME_SUFFIX_LENGTH, &super::ID_ALPHABET)
    );

    users::ActiveModel {
        id: Set(id),
        email: Set(params.email),
        passhash: Set(Some(pass_hash)),
        username: Set(username),
        ..Default::default()
    }
    .insert(&state.conn)
    .await
}

pub async fn update_user(
    state: &AppState,
    id: &str,
    params: UpdateUserParams,
) -> Result<users::Model, DbErr> {
    let mut update_query = users::Entity::update_many().filter(users::Column::Id.eq(id));

    if let Some(username) = params.username {
        update_query = update_query.col_expr(
            users::Column::Username,
            sea_orm::sea_query::Expr::value(username),
        );
    }

    update_query.exec(&state.conn).await?;

    users::Entity::find_by_id(id)
        .one(&state.conn)
        .await?
        .ok_or(DbErr::RecordNotFound(
            "User not found after update".to_string(),
        ))
}

pub async fn search_users(state: &AppState, query: UserQuery) -> Result<Vec<users::Model>, DbErr> {
    users::Entity::find()
        .filter(users::Column::Username.contains(query.username))
        .all(&state.conn)
        .await
}

pub async fn get_user(state: &AppState, id: &str) -> Result<Option<users::Model>, DbErr> {
    users::Entity::find_by_id(id).one(&state.conn).await
}

pub async fn login_user(
    state: &AppState,
    LoginUserParams { email, password }: LoginUserParams,
) -> Result<users::Model, DbErr> {
    let user = match users::Entity::find()
        .filter(users::Column::Email.eq(&email))
        .one(&state.conn)
        .await?
    {
        None => return Err(DbErr::RecordNotFound("User not found".to_string())),
        Some(user) => user,
    };

    if let Some(passhash) = &user.passhash {
        if !bcrypt::verify(password, &passhash).unwrap() {
            return Err(DbErr::Custom("Password incorrect".to_string()));
        }
    } else {
        return Err(DbErr::Custom("Password incorrect".to_string()));
    }

    Ok(user)
}

pub async fn forgot_password(
    state: &AppState,
    body: ForgotPasswordBody,
) -> Result<models::domains::otp::Model, anyhow::Error> {
    let now = Utc::now();
    let db = &state.conn;
    let email = body.email.trim().to_lowercase();

    let user = users::Entity::find()
        .filter(users::Column::Email.eq(email.clone()))
        .one(db)
        .await?;

    if user.is_none() {
        return Err(anyhow::anyhow!("User not found"));
    }

    match otp::Entity::find_by_id(email.clone())
        .one(&state.conn)
        .await?
    {
        Some(existing_otp)
            if now.signed_duration_since(existing_otp.created_at) <= OTP_DURATION =>
        {
            return Ok(existing_otp);
        }
        Some(_) => {
            // If OTP exists but is expired, delete it
            otp::Entity::delete_by_id(email.clone())
                .exec(&state.conn)
                .await?;
        }
        _ => {}
    }

    let mut rng = StdRng::from_os_rng();

    let otp_value = rng.random_range(100000..1000000);

    let otp = models::domains::otp::Model {
        email: email.clone(),
        otp: otp_value,
        created_at: Utc::now().fixed_offset(),
    };

    let otp_model = otp::ActiveModel {
        email: Set(email.clone()),
        otp: Set(otp_value),
        created_at: Set(otp.created_at),
    };

    otp_model.insert(&state.conn).await?;

    Ok(otp)
}

pub async fn reset_password(state: &AppState, params: ResetPasswordBody) -> Result<String, DbErr> {
    use chrono::Utc;
    use models::domains::otp;
    use sea_orm::TransactionTrait;

    let txn = state.conn.begin().await?;

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
    if now.signed_duration_since(otp_record.created_at) > OTP_DURATION {
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
    user.passhash = Set(Some(pass_hash));
    user.update(&txn).await?;

    // 5. Delete OTP
    otp::Entity::delete_by_id(params.email.clone())
        .exec(&txn)
        .await?;

    // 6. Commit transaction
    txn.commit().await?;
    Ok("Password reset successful".into())
}

pub async fn email_auth(state: &AppState, params: EmailAuthParams) -> Result<users::Model, DbErr> {
    use sea_orm::TransactionTrait;
    // Start a transaction with the highest isolation level to prevent race conditions.
    // Serializable makes the transaction behave as if it's the only one running.
    let txn = state.conn.begin().await?;

    // --- The entire operation now happens within the transaction ---

    // 1. Check for the user *within* the transaction.
    let existing_user = users::Entity::find()
        .filter(users::Column::Email.eq(&params.email))
        .one(&txn) // Use the transaction connection `&txn`
        .await?;

    let user_model = if let Some(user) = existing_user {
        // --- CASE 1: USER EXISTS ---
        // The user was found. We don't need to do anything else.
        user
    } else {
        // --- CASE 2: NEW USER ---
        // No user was found, so we create one within the same transaction.
        let id = nanoid::nanoid!(USER_ID_LENGTH, &super::ID_ALPHABET);
        let username = format!(
            "{}{}",
            faker::internet::en::Username().fake::<String>(),
            nanoid::nanoid!(USERNAME_SUFFIX_LENGTH, &super::ID_ALPHABET)
        );

        users::ActiveModel {
            id: Set(id),
            email: Set(params.email),
            username: Set(username),
            passhash: Set(None),
            ..Default::default()
        }
        .insert(&txn)
        .await?
    };

    txn.commit().await?;

    Ok(user_model)
}
