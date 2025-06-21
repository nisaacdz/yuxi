use app::persistence::users::{create_user, login_user};
use app::state::AppState;
use app::utils::{encode_data, send_forgot_password_email};
use axum::{
    Extension, Router,
    extract::State,
    response::IntoResponse,
    routing::{get, post},
};
use models::params::user::{ForgotPasswordBody, ResetPasswordBody};
use models::schemas::user::{AuthSchema, LoginSchema, TokensSchema};
use models::{
    params::user::{CreateUserParams, LoginUserParams},
    schemas::user::UserSchema,
};

use crate::ApiResponse;
use crate::error::ApiError;
use crate::extractor::Json;

#[axum::debug_handler]
pub async fn login_post(
    State(state): State<AppState>,
    Json(params): Json<LoginUserParams>,
) -> Result<impl IntoResponse, ApiError> {
    let user = login_user(&state.conn, params)
        .await
        .map_err(ApiError::from)?;

    let access = encode_data(&state.config, &UserSchema::from(user.clone()))?;

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
    Json(params): Json<CreateUserParams>,
) -> Result<impl IntoResponse, ApiError> {
    let user = create_user(&state.conn, params)
        .await
        .map(UserSchema::from)
        .map_err(ApiError::from)?;

    let access = encode_data(&state.config, &user)?;
    let tokens = TokensSchema { access };
    let login_response = LoginSchema { user, tokens };

    let response = ApiResponse::success("Registration successful", Some(login_response));

    Ok(Json(response))
}

#[axum::debug_handler]
pub async fn me_get(
    Extension(client_state): Extension<AuthSchema>,
) -> Result<impl IntoResponse, ApiError> {
    let response = ApiResponse::success("User data retrieved", Some(client_state));
    Ok(Json(response))
}

#[axum::debug_handler]
pub async fn forgot_password_post(
    State(state): State<AppState>,
    Json(params): Json<ForgotPasswordBody>,
) -> Result<impl IntoResponse, ApiError> {
    let email = params.email.trim().to_lowercase();
    let otp = app::persistence::users::forgot_password(&state, params)
        .await
        .map_err(ApiError::from)?;

    send_forgot_password_email(&state.config, &email, &otp.otp.to_string()).await?;

    let response = ApiResponse::success("OTP sent to email", Some("OTP sent successfully"));
    Ok(Json(response))
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
