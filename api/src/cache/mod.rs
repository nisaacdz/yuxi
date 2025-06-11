use std::sync::Arc;
use app::cache::Cache;

use models::schemas::typing::TypingSessionSchema;

use crate::action::manager::TournamentManager;

#[derive(Clone)]
pub struct TournamentRegistry {
    registry: Cache<Arc<TournamentManager>>,
}

impl TournamentRegistry {
    pub fn new() -> Self {
        Self {
            registry: Cache::new(),
        }
    }

    pub fn get_or_init<F>(&self, tournament_id: String, with: F) -> Arc<TournamentManager>
    where
        F: FnOnce() -> TournamentManager,
    {
        self.registry
            .get_or_insert(&tournament_id, || Arc::new(with()))
    }

    pub fn evict(&self, tournament_id: &str) -> Option<Arc<TournamentManager>> {
        self.registry.delete_data(tournament_id)
    }
}

#[derive(Clone)]
pub struct TypingSessionRegistry {
    sessions: Cache<TypingSessionSchema>,
}

impl TypingSessionRegistry {
    pub fn new() -> Self {
        Self {
            sessions: Cache::new(),
        }
    }

    pub fn contains_session(&self, client_id: &str) -> bool {
        self.sessions.contains_key(client_id)
    }

    pub fn get_session(&self, client_id: &str) -> Option<TypingSessionSchema> {
        self.sessions.get_data(client_id)
    }

    pub fn set_session(&self, client_id: &str, session: TypingSessionSchema) {
        self.sessions.set_data(client_id, session);
    }

    pub fn delete_session(&self, client_id: &str) -> Option<TypingSessionSchema> {
        self.sessions.delete_data(client_id)
    }
}
