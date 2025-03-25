use serde::Serialize;

use crate::domains::users;

#[derive(Serialize)]
pub struct UserSchema {
    pub id: i32,
    pub username: String,
    pub email: String,
}

impl From<users::Model> for UserSchema {
    fn from(user: users::Model) -> Self {
        Self {
            id: user.id,
            username: user.username,
            email: user.email,
        }
    }
}

#[derive(Serialize)]
pub struct UserListSchema {
    pub users: Vec<UserSchema>,
}

impl From<Vec<users::Model>> for UserListSchema {
    fn from(users: Vec<users::Model>) -> Self {
        Self {
            users: users.into_iter().map(UserSchema::from).collect(),
        }
    }
}
