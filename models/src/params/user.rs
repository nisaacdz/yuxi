use serde::Deserialize;
use validator::Validate;

#[derive(Deserialize, Validate)]
pub struct CreateUserParams {
    #[validate(length(min = 2))]
    pub username: String,
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 8))]
    pub password: String,
}

#[derive(Deserialize, Validate)]
pub struct LoginUserParams {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 8))]
    pub password: String,
}

// update later with more fields
#[derive(Deserialize, Validate)]
pub struct UpdateUserParams {
    #[validate(length(min = 2))]
    pub username: Option<String>,
}
