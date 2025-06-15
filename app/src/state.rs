use sea_orm::DatabaseConnection;
use socketioxide::SocketIo;

use crate::{
    cache::{TournamentRegistry, TypingSessionRegistry},
    config::Config,
};

#[derive(Clone)]
pub struct AppState {
    pub conn: DatabaseConnection,
    pub config: Config,
    pub tournament_registry: TournamentRegistry,
    pub typing_session_registry: TypingSessionRegistry,
    pub socket_io: SocketIo,
}
