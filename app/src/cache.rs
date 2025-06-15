use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
};

use models::schemas::typing::TypingSessionSchema;

use crate::core::TournamentManager;

pub struct Cache<T> {
    data: Arc<Mutex<HashMap<String, T>>>,
}

impl<T> Clone for Cache<T> {
    fn clone(&self) -> Self {
        Cache {
            data: Arc::clone(&self.data),
        }
    }
}

impl<T> Cache<T> {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    fn get_connection(&self) -> MutexGuard<'_, HashMap<String, T>> {
        self.data.lock().unwrap()
    }

    pub fn set_data(&self, id: &str, data: T) {
        let mut conn = self.get_connection();
        conn.insert(id.to_owned(), data);
    }

    pub fn contains_key(&self, id: &str) -> bool {
        let conn = self.get_connection();
        conn.contains_key(id)
    }

    pub fn update_data<F, O>(&self, id: &str, update: F) -> Option<O>
    where
        F: FnOnce(&mut T) -> O,
    {
        let mut conn = self.get_connection();
        conn.get_mut(id).map(|data| update(data))
    }

    pub fn delete_data(&self, id: &str) -> Option<T> {
        let mut conn = self.get_connection();
        conn.remove(id)
    }

    pub fn values(&self) -> Vec<T>
    where
        T: Clone,
    {
        let conn = self.get_connection();
        conn.values().cloned().collect()
    }

    pub fn keys(&self) -> Vec<String> {
        let conn = self.get_connection();
        conn.keys().cloned().collect()
    }

    pub fn count(&self) -> usize {
        self.get_connection().len()
    }
}

impl<T: Clone> Cache<T> {
    pub fn get_data(&self, id: &str) -> Option<T> {
        let conn = self.get_connection();
        conn.get(id).map(|data| data.clone())
    }

    pub fn get_or_insert<F>(&self, id: &str, with: F) -> T
    where
        F: FnOnce() -> T,
    {
        let mut conn = self.get_connection();
        conn.entry(id.to_owned()).or_insert_with(with).clone()
    }
}

impl<T> Default for Cache<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct TournamentRegistry {
    registry: Cache<TournamentManager>,
}

impl TournamentRegistry {
    pub fn new() -> Self {
        Self {
            registry: Cache::new(),
        }
    }

    pub fn get(&self, id: &str) -> Option<TournamentManager> {
        self.registry.get_data(id)
    }

    pub fn get_or_init<F>(&self, tournament_id: String, with: F) -> TournamentManager
    where
        F: FnOnce() -> TournamentManager,
    {
        self.registry.get_or_insert(&tournament_id, || with())
    }

    pub fn evict(&self, tournament_id: &str) -> Option<TournamentManager> {
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
