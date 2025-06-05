use app::persistence::users::{create_user, login_user};
use app::state::AppState;
use app::utils::encode_data;
use axum::{
    Extension, Router,
    extract::State,
    response::IntoResponse,
    routing::{get, post},
};
use chrono::Utc;
use models::params::user::{ForgotPasswordBody, ResetPasswordBody};
use models::schemas::user::{ClientSchema, LoginSchema, TokensSchema};
use models::{
    params::user::{CreateUserParams, LoginUserParams},
    schemas::user::UserSchema,
};
use sea_orm::TryIntoModel;

use crate::ApiResponse;
use crate::error::ApiError;
use crate::extractor::Json;

#[axum::debug_handler]
pub async fn login_post(
    State(state): State<AppState>,
    Extension(client): Extension<ClientSchema>,
    Json(params): Json<LoginUserParams>,
) -> Result<impl IntoResponse, ApiError> {
    let user = login_user(&state.conn, params)
        .await
        .map_err(ApiError::from)?;

    let access = encode_data(
        &state.config,
        &ClientSchema {
            id: client.id.clone(),
            user: Some(UserSchema::from(user.clone())),
            updated: Utc::now(),
        },
    )?;

    let user_schema = UserSchema::from(user);
    let tokens = TokensSchema { access };
    let login_response = LoginSchema {
        user: user_schema,
        tokens,
    };

    let response = ApiResponse::success("Login successful", Some(login_response));

    Ok(Json(response))
}

#[axum::debug_handler]
pub async fn register_post(
    State(state): State<AppState>,
    Extension(client): Extension<ClientSchema>,
    Json(params): Json<CreateUserParams>,
) -> Result<impl IntoResponse, ApiError> {
    let user_db = create_user(&state.conn, params)
        .await
        .map_err(ApiError::from)?
        .try_into_model()
        .map_err(ApiError::from)?;

    let updated_client_state = ClientSchema {
        id: client.id,
        user: Some(UserSchema::from(user_db.clone())),
        updated: Utc::now(),
    };

    let access = encode_data(&state.config, &updated_client_state)?;
    let tokens = TokensSchema { access };
    let login_response = LoginSchema {
        user: UserSchema::from(user_db),
        tokens,
    };

    let response = ApiResponse::success("Registration successful", Some(login_response));

    Ok(Json(response))
}

#[axum::debug_handler]
pub async fn me_get(
    Extension(client_state): Extension<ClientSchema>,
) -> Result<impl IntoResponse, ApiError> {
    let response = ApiResponse::success("User data retrieved", Some(client_state));
    Ok(Json(response))
}

#[axum::debug_handler]
pub async fn forgot_password_post(
    State(state): State<AppState>,
    Json(params): Json<ForgotPasswordBody>,
) -> Result<impl IntoResponse, ApiError> {
    // Involves creating an OTP and sending it to the user's email
    // No new OTP is created if one already exists
    // We need to configure the expiration time for the OTP in our Config
    Err(ApiError::NotImplemented(
        "Forgot password not implemented".to_string(),
    ))
}

#[axum::debug_handler]
pub async fn reset_password_post(
    State(state): State<AppState>,
    Json(body): Json<ResetPasswordBody>,
) -> Result<impl IntoResponse, ApiError> {
    let result = app::persistence::users::reset_password(&state.conn, body)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(ApiResponse::success(
        "Password reset successful",
        Some(result),
    )))
}

pub fn create_auth_router() -> Router<AppState> {
    Router::new()
        .route("/login", post(login_post))
        .route("/register", post(register_post))
        .route("/me", get(me_get))
        .route("/forgot-password", post(forgot_password_post))
        .route("/reset-password", post(reset_password_post))
}
