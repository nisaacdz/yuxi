use serde::Serialize;
use utoipa::ToSchema;

use crate::domains::user;

#[derive(Serialize, ToSchema)]
pub struct UserSchema {
    pub username: String,
    pub email: String,
}

impl From<user::Model> for UserSchema {
    fn from(user: user::Model) -> Self {
        Self {
            username: user.username,
            email: user.email,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct UserListSchema {
    pub users: Vec<UserSchema>,
}

impl From<Vec<user::Model>> for UserListSchema {
    fn from(users: Vec<user::Model>) -> Self {
        Self {
            users: users.into_iter().map(UserSchema::from).collect(),
        }
    }
}
