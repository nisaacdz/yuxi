use models::domains::*;
use sea_orm::DatabaseConnection;
use socketioxide::SocketIo;

type User = users::Model;
type Tournament = tournaments::Model;
type Otp = otp::Model;
type TypingHistory = typing_history::Model;

use crate::{
    cache::{Cache, TournamentRegistry, TypingSessionRegistry},
    config::Config,
};

#[derive(Clone)]
pub struct AppState {
    pub tables: Tables,
    pub config: Config,
    pub tournament_registry: TournamentRegistry,
    pub typing_session_registry: TypingSessionRegistry,
    pub socket_io: SocketIo,
}

#[derive(Clone)]
pub struct Tables {
    pub users: Cache<User>,
    pub tournaments: Cache<Tournament>,
    pub otps: Cache<Otp>,
    pub history: Cache<TypingHistory>,
}

impl Tables {
    pub fn new() -> Self {
        Self {
            users: Cache::new(),
            tournaments: Cache::new(),
            otps: Cache::new(),
            history: Cache::new(),
        }
    }
}
