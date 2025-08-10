use serde::Deserialize;
use validator::Validate;

#[derive(Deserialize, Validate, Debug)]
pub struct CreateUserParams {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 8))]
    pub password: String,
}

#[derive(Deserialize)]
pub struct GoogleAuthParams {
    pub code: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailAuthParams {
    pub email: String,
}

#[derive(Deserialize, Validate)]
pub struct LoginUserParams {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 8))]
    pub password: String,
}

#[derive(Deserialize, Validate)]
pub struct UpdateUserParams {
    #[validate(length(min = 2))]
    pub username: Option<String>,
}

#[derive(Deserialize, Validate)]
pub struct ResetPasswordBody {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 6, max = 6))]
    pub otp: String,
    #[validate(length(min = 8))]
    pub password: String,
}

#[derive(Deserialize, Validate)]
pub struct ForgotPasswordBody {
    #[validate(email)]
    pub email: String,
}
