use serde::{Deserialize, Serialize};

use crate::domains::users;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AuthSchema {
    pub user: Option<UserSchema>,
}

impl AuthSchema {
    pub fn new(user: Option<users::Model>) -> Self {
        Self {
            user: user.map(UserSchema::from),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TournamentRoomUserProfile { 
    pub username: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TournamentRoomMember {
    pub id: String,
    pub user: Option<TournamentRoomUserProfile>,
}

impl TournamentRoomMember {
    pub fn from_user(user: &UserSchema, anonymous: bool) -> Self {
        Self {
            id: TournamentRoomMember::get_id(&user.id),
            user: if anonymous { None } else { Some(TournamentRoomUserProfile { username: user.username.clone() }) }
        }
    }

    // TODO implement a more secure transformation
    pub fn get_id(user_id: &str) -> String {
        user_id.to_owned()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UserSchema {
    pub id: String,
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LoginSchema {
    pub user: UserSchema,
    pub tokens: TokensSchema,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TokensSchema {
    pub access: String,
}
