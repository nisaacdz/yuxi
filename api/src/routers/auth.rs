use app::persistence::users::{create_user, login_user};
use app::state::AppState;
use app::utils::{encode_data, send_forgot_password_email};
use axum::{
    Extension, Router,
    extract::State,
    response::IntoResponse,
    routing::{get, post},
};
use models::params::user::{
    AuthCodeParams, EmailAuthParams, ForgotPasswordBody, ResetPasswordBody,
};
use models::schemas::user::{AuthSchema, LoginSchema, TokensSchema};
use models::{
    params::user::{CreateUserParams, LoginUserParams},
    schemas::user::UserSchema,
};

use anyhow::anyhow;
//use openidconnect::core::CoreGenderClaim;
use openidconnect::{
    AuthorizationCode, /*EmptyAdditionalClaims,*/ Nonce,
    /*OAuth2TokenResponse,*/ TokenResponse,
};

use crate::ApiResponse;
use crate::error::ApiError;
use crate::extractor::Json;

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    tag = "auth",
    request_body = LoginUserParams,
    responses(
        (status = 200, description = "Login successful", body = ApiResponse<LoginSchema>),
        (status = 401, description = "Invalid credentials"),
    )
)]
#[axum::debug_handler]
pub async fn login_post(
    State(state): State<AppState>,
    Json(params): Json<LoginUserParams>,
) -> Result<impl IntoResponse, ApiError> {
    let user = login_user(&state, params).await.map_err(ApiError::from)?;

    let access = encode_data(&state.config, UserSchema::from(user.clone()))?;

    let user_schema = UserSchema::from(user);
    let tokens = TokensSchema { access };
    let login_response = LoginSchema {
        user: user_schema,
        tokens,
    };

    let response = ApiResponse::success("Login successful", Some(login_response));

    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/register",
    tag = "auth",
    request_body = CreateUserParams,
    responses(
        (status = 200, description = "Registration successful", body = ApiResponse<LoginSchema>),
        (status = 400, description = "Invalid input or user already exists"),
    )
)]
#[axum::debug_handler]
pub async fn register_post(
    State(state): State<AppState>,
    Json(params): Json<CreateUserParams>,
) -> Result<impl IntoResponse, ApiError> {
    let user = create_user(&state, params)
        .await
        .map(UserSchema::from)
        .map_err(ApiError::from)?;

    let access = encode_data(&state.config, &user)?;
    let tokens = TokensSchema { access };
    let login_response = LoginSchema { user, tokens };

    let response = ApiResponse::success("Registration successful", Some(login_response));

    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/me",
    tag = "auth",
    responses(
        (status = 200, description = "User data retrieved", body = ApiResponse<AuthSchema>),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
#[axum::debug_handler]
pub async fn me_get(
    Extension(auth_state): Extension<AuthSchema>,
) -> Result<impl IntoResponse, ApiError> {
    let response = ApiResponse::success("User data retrieved", Some(auth_state));
    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/forgot-password",
    tag = "auth",
    request_body = ForgotPasswordBody,
    responses(
        (status = 200, description = "OTP sent to email"),
        (status = 404, description = "User not found"),
    )
)]
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

#[utoipa::path(
    post,
    path = "/api/v1/auth/reset-password",
    tag = "auth",
    request_body = ResetPasswordBody,
    responses(
        (status = 200, description = "Password reset successful"),
        (status = 400, description = "Invalid OTP or request"),
    )
)]
#[axum::debug_handler]
pub async fn reset_password_post(
    State(state): State<AppState>,
    Json(body): Json<ResetPasswordBody>,
) -> Result<impl IntoResponse, ApiError> {
    let result = app::persistence::users::reset_password(&state, body)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(ApiResponse::success(
        "Password reset successful",
        Some(result),
    )))
}

fn empty_nonce_verifier(_: Option<&Nonce>) -> Result<(), String> {
    Ok(())
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/google",
    tag = "auth",
    request_body = AuthCodeParams,
    responses(
        (status = 200, description = "Google authentication successful", body = ApiResponse<LoginSchema>),
        (status = 400, description = "Invalid authorization code"),
    )
)]
#[axum::debug_handler]
pub async fn google_auth_post(
    State(state): State<AppState>,
    Json(params): Json<AuthCodeParams>,
) -> Result<impl IntoResponse, ApiError> {
    let google_auth_client = &state.config.google_auth_client;
    let http_client = &state.config.http_client;
    let token_response = google_auth_client
        .exchange_code(AuthorizationCode::new(params.code))
        .map_err(|err| {
            eprintln!("Google token exchange error: {:?}", err);
            anyhow!("Something went wrong!")
        })?
        .request_async(http_client)
        .await
        .map_err(|err| {
            eprintln!("Google token exchange error: {:?}", err);
            anyhow!("An error occurred during Google token exchange")
        })?;

    let id_token = token_response
        .id_token()
        .ok_or_else(|| anyhow!("ID token not found"))?;

    let claims = id_token.claims(
        &google_auth_client.id_token_verifier(),
        empty_nonce_verifier,
    )?;

    let user_info: EmailAuthParams = serde_json::from_str(&serde_json::to_string(&claims)?)?;

    let user = app::persistence::users::email_auth(&state, user_info).await?;

    let user_schema = UserSchema::from(user);
    let access = app::utils::encode_data(&state.config, &user_schema)?;
    let tokens = TokensSchema { access };
    let login_response = LoginSchema {
        user: user_schema,
        tokens,
    };

    let response = ApiResponse::success("Login successful", Some(login_response));

    Ok(Json(response))
}

// #[axum::debug_handler]
// pub async fn facebook_auth_post(
//     State(state): State<AppState>,
//     Json(params): Json<AuthCodeParams>,
// ) -> Result<impl IntoResponse, ApiError> {
//     let facebook_auth_client = &state.config.facebook_auth_client;
//     let http_client = &state.config.http_client;

//     let token_response = facebook_auth_client
//         .exchange_code(AuthorizationCode::new(params.code))
//         .map_err(|err| {
//             eprintln!("Facebook token exchange error: {:?}", err);
//             anyhow!("Something went wrong!")
//         })?
//         .request_async(http_client)
//         .await
//         .map_err(|err| {
//             eprintln!("Facebook token exchange error: {:?}", err);
//             anyhow!("An error occurred during Facebook token exchange")
//         })?;

//     // With OpenID Connect, user information is retrieved from the UserInfo endpoint.
//     let claims = facebook_auth_client
//         .user_info(token_response.access_token().clone(), None)
//         .map_err(|err| {
//             eprintln!("Failed to prepare user info request: {:?}", err);
//             anyhow!("Something went wrong!")
//         })?
//         .request_async::<EmptyAdditionalClaims, _, CoreGenderClaim>(http_client)
//         .await
//         .map_err(|err| {
//             eprintln!("Facebook user info error: {:?}", err);
//             anyhow!("Failed to get user info from Facebook")
//         })?;

//     let user_info: EmailAuthParams = serde_json::from_value(serde_json::to_value(claims)?)?;

//     let user = app::persistence::users::email_auth(&state, user_info).await?;

//     let user_schema = UserSchema::from(user);
//     let access = app::utils::encode_data(&state.config, &user_schema)?;
//     let tokens = TokensSchema { access };
//     let login_response = LoginSchema {
//         user: user_schema,
//         tokens,
//     };

//     let response = ApiResponse::success("Login successful", Some(login_response));

//     Ok(Json(response))
// }

pub fn create_auth_router() -> Router<AppState> {
    Router::new()
        .route("/login", post(login_post))
        .route("/register", post(register_post))
        .route("/me", get(me_get))
        .route("/forgot-password", post(forgot_password_post))
        .route("/reset-password", post(reset_password_post))
        .route("/google", post(google_auth_post))
    //.route("/facebook", post(facebook_auth_post))
}
