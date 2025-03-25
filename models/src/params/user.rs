use serde::Deserialize;
use validator::Validate;

#[derive(Deserialize, Validate)]
pub struct CreateUserParams {
    #[validate(length(min = 2))]
    pub username: String,
}
