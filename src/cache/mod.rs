use std::{any::Any, collections::HashMap};

use lazy_static::lazy_static;
use models::schemas::{tournament::TournamentInfo, typing::TypingSession};
use redis::{AsyncCommands, Client};
use tokio::sync::{Mutex, MutexGuard};

//static REDIS_CLIENT: OnceCell<Client> = OnceCell::const_new();

type STORAGE = HashMap<String, Box<dyn Any + Send + Sync>>;
lazy_static::lazy_static! {
    static ref FAKE_REDIS_CLIENT: Mutex<STORAGE> =
        Mutex::new(HashMap::new());
}

pub async fn get_redis_connection() -> MutexGuard<'static, STORAGE> {
    FAKE_REDIS_CLIENT.lock().await
}

pub async fn redis_get_tournament(tournament_id: &str) -> Option<TournamentInfo> {
    let conn = get_redis_connection().await;
    let value = conn.get(&format!("T-{}", tournament_id));
    // cast to TournamentInfo
    value
        .map(|v| v.downcast_ref::<TournamentInfo>())
        .flatten()
        .map(|v| v.clone())
}

pub async fn redis_set_tournament(tournament_id: &str, tournament: TournamentInfo) {
    let mut conn = get_redis_connection().await;
    conn.insert(format!("T-{}", tournament_id), Box::new(tournament));
}

pub async fn redis_update_tournament(tournament_id: &str, update: TournamentInfo) {
    let mut conn = get_redis_connection().await;
    conn.entry(format!("T-{}", tournament_id))
        .insert_entry(Box::new(update));
}

pub async fn redis_delete_tournament(tournament_id: &str) {
    let mut conn = get_redis_connection().await;
    conn.remove(&format!("T-{}", tournament_id));
}

pub async fn redis_get_typing_session(client_id: &str) -> Option<TypingSession> {
    let conn = get_redis_connection().await;
    let value = conn.get(&format!("TS-{}", client_id));
    // cast to TypingSession
    value
        .map(|v| v.downcast_ref::<TypingSession>())
        .flatten()
        .map(|v| v.clone())
}

pub async fn redis_set_typing_session(client_id: &str, session: TypingSession) {
    let mut conn = get_redis_connection().await;
    conn.insert(format!("TS-{}", client_id), Box::new(session));
}

pub async fn redis_delete_typing_session(_tournament_id: &str, client_id: &str) {
    let mut conn = get_redis_connection().await;
    conn.remove(&format!("TS-{}", client_id));
}

pub fn initialize_redis(_redis_url: &String) {
    // Do nothing, all okay
}
