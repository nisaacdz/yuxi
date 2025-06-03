use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domains::users;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ClientSchema {
    pub id: String,
    pub user: Option<UserSchema>,
    pub updated: DateTime<Utc>,
}

impl ClientSchema {
    pub fn update(&mut self, user_model: Option<users::Model>) {
        self.user = user_model.map(UserSchema::from);
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LoginSchema {
    pub user: UserSchema,
    pub tokens: TokensSchema,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TokensSchema {
    pub access: String,
}
