use serde::{Deserialize, Serialize};
pub mod handlers;
pub mod moderation;
pub mod registry;
pub mod state;

pub mod manager;

pub mod timeout;

#[derive(Deserialize, Clone, Debug)]
pub struct TypeArgs {
    pub character: char,
}
