use fake::{Fake, faker};
use rand::Rng;
use rand::{SeedableRng, rngs::StdRng};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, EntityTrait, IntoActiveModel, QueryFilter, Set,
};

use models::domains::{otp, users};
use models::params::user::{
    CreateUserParams, ForgotPasswordBody, LoginUserParams, ResetPasswordBody, UpdateUserParams,
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

    let existing_user = state
        .tables
        .users
        .values()
        .into_iter()
        .find(|u| &u.email == &params.email);

    if existing_user.is_some() {
        return Err(DbErr::Custom("User already exists".to_string()));
    }

    let id = nanoid::nanoid!(USER_ID_LENGTH, &super::ID_ALPHABET);

    let username = format!(
        "{}{}",
        faker::internet::en::Username().fake::<String>(),
        nanoid::nanoid!(USERNAME_SUFFIX_LENGTH, &super::ID_ALPHABET)
    );

    let new_user = users::Model {
        id: id.clone(),
        email: params.email,
        passhash: pass_hash,
        username: username,
        created_at: Utc::now().fixed_offset(),
        updated_at: Utc::now().fixed_offset(),
    };

    state.tables.users.set_data(&id, new_user.clone());

    Ok(new_user)
}

pub async fn update_user(
    state: &AppState,
    id: &str,
    params: UpdateUserParams,
) -> Result<users::Model, DbErr> {
    state.tables.users.update_data(id, |u| {
        if let Some(username) = params.username {
            u.username = username;
        }
    });

    state
        .tables
        .users
        .get_data(id)
        .ok_or_else(|| DbErr::RecordNotFound("user not found".into()))
}

pub async fn search_users(db: &DbConn, query: UserQuery) -> Result<Vec<users::Model>, DbErr> {
    users::Entity::find()
        .filter(users::Column::Username.contains(query.username))
        .all(db)
        .await
}

pub async fn get_user(state: &AppState, id: &str) -> Result<Option<users::Model>, DbErr> {
    Ok(state.tables.users.get_data(id))
}

pub async fn login_user(
    state: &AppState,
    LoginUserParams { email, password }: LoginUserParams,
) -> Result<users::Model, DbErr> {
    let user = match state
        .tables
        .users
        .values()
        .into_iter()
        .find(|u| &u.email == &email)
    {
        None => return Err(DbErr::RecordNotFound("User not found".to_string())),
        Some(user) => user,
    };
    if !bcrypt::verify(password, &user.passhash).unwrap() {
        return Err(DbErr::Custom("Password incorrect".to_string()));
    }
    Ok(user)
}

pub async fn forgot_password(
    state: &AppState,
    body: ForgotPasswordBody,
) -> Result<models::domains::otp::Model, anyhow::Error> {
    let now = Utc::now();
    let email = body.email.trim().to_lowercase();

    let user = state
        .tables
        .users
        .values()
        .into_iter()
        .find(|u| email == u.email);

    if user.is_none() {
        return Err(anyhow::anyhow!("User not found"));
    }

    match state.tables.otps.get_data(&email) {
        Some(existing_otp)
            if now.signed_duration_since(existing_otp.created_at) <= OTP_DURATION =>
        {
            return Ok(existing_otp);
        }
        Some(_) => {
            // If OTP exists but is expired, delete it
            state.tables.otps.delete_data(&email);
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

    let otp_model = otp::Model {
        email: email.clone(),
        otp: otp_value,
        created_at: otp.created_at,
    };

    state.tables.otps.set_data(&email, otp_model);

    Ok(otp)
}

pub async fn reset_password(state: &AppState, params: ResetPasswordBody) -> Result<String, DbErr> {
    use chrono::Utc;
    use models::domains::otp;
    // 1. Find OTP record for the email
    let otp_record = state.tables.otps.get_data(&params.email);
    let otp_record = match otp_record {
        Some(r) => r,
        None => {
            return Err(DbErr::Custom("OTP not found".to_string()));
        }
    };

    // 2. Check OTP matches and is not expired (10 min window)
    let now = Utc::now();
    if otp_record.otp.to_string() != params.otp {
        return Err(DbErr::Custom("OTP incorrect".to_string()));
    }

    if now.signed_duration_since(otp_record.created_at) > OTP_DURATION {
        return Err(DbErr::Custom("OTP expired".to_string()));
    }

    // 3. Hash new password
    let pass_hash = bcrypt::hash(params.password, 4).map_err(|e| DbErr::Custom(e.to_string()))?;

    // 4. Update user's password
    let user = state
        .tables
        .users
        .values()
        .into_iter()
        .find(|u| u.email == params.email)
        .expect("User is missing");
    let user = state
        .tables
        .users
        .update_data(&user.id, |u| u.passhash == pass_hash);

    // 5. Delete OTP
    state.tables.otps.delete_data(&params.email);
    // 6. Commit transaction
    Ok("Password reset successful".into())
}
