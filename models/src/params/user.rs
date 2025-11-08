use serde::Deserialize;
use utoipa::ToSchema;
use validator::Validate;

#[derive(Deserialize, Validate, Debug, ToSchema)]
pub struct CreateUserParams {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 8))]
    pub password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct AuthCodeParams {
    pub code: String,
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EmailAuthParams {
    pub email: String,
}

#[derive(Deserialize, Validate, ToSchema)]
pub struct LoginUserParams {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 8))]
    pub password: String,
}

#[derive(Deserialize, Validate, ToSchema)]
pub struct UpdateUserParams {
    #[validate(length(min = 2))]
    pub username: Option<String>,
}

#[derive(Deserialize, Validate, ToSchema)]
pub struct ResetPasswordBody {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 6, max = 6))]
    pub otp: String,
    #[validate(length(min = 8))]
    pub password: String,
}

#[derive(Deserialize, Validate, ToSchema)]
pub struct ForgotPasswordBody {
    #[validate(email)]
    pub email: String,
}
