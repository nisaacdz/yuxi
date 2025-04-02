use std::{any::Any, collections::HashMap};

use models::schemas::{tournament::TournamentInfo, typing::TypingSessionSchema};
use tokio::sync::{Mutex, MutexGuard};

type STORAGE = HashMap<String, Box<dyn Any + Send + Sync>>;
lazy_static::lazy_static! {
    static ref CACHE_STORE: Mutex<STORAGE> =
        Mutex::new(HashMap::new());
}

pub async fn get_cache_connection() -> MutexGuard<'static, STORAGE> {
    CACHE_STORE.lock().await
}

pub async fn cache_get_tournament(tournament_id: &str) -> Option<TournamentInfo> {
    let conn = get_cache_connection().await;
    let value = conn.get(&format!("T-{}", tournament_id));
    // cast to TournamentInfo
    value
        .map(|v| v.downcast_ref::<TournamentInfo>())
        .flatten()
        .map(|v| v.clone())
}

pub async fn cache_set_tournament(tournament_id: &str, tournament: TournamentInfo) {
    let mut conn = get_cache_connection().await;
    conn.insert(format!("T-{}", tournament_id), Box::new(tournament));
}

pub async fn cache_update_tournament(
    tournament_id: &str,
    update: impl FnOnce(&mut TournamentInfo),
) {
    let mut conn = get_cache_connection().await;
    if let Some(tournament) = conn.get_mut(&format!("T-{}", tournament_id)) {
        let tournament = tournament.downcast_mut::<TournamentInfo>().unwrap();
        update(tournament);
    }
}

pub async fn cache_delete_tournament(tournament_id: &str) {
    let mut conn = get_cache_connection().await;
    conn.remove(&format!("T-{}", tournament_id));
}

pub async fn cache_get_typing_session(client_id: &str) -> Option<TypingSessionSchema> {
    let conn = get_cache_connection().await;
    let value = conn.get(&format!("TS-{}", client_id));
    // cast to TypingSessionSchema
    value
        .map(|v| v.downcast_ref::<TypingSessionSchema>())
        .flatten()
        .map(|v| v.clone())
}

pub async fn cache_set_typing_session(client_id: &str, session: TypingSessionSchema) {
    let mut conn = get_cache_connection().await;
    conn.insert(format!("TS-{}", client_id), Box::new(session));
}

pub async fn cache_delete_typing_session(_tournament_id: &str, client_id: &str) {
    let mut conn = get_cache_connection().await;
    conn.remove(&format!("TS-{}", client_id));
}

pub fn initialize_cache(_cache_url: &String) {
    // Do nothing, all okay
}
