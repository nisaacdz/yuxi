use std::{any::Any, collections::HashMap};

use models::schemas::{tournament::TournamentSession, typing::TypingSessionSchema};
use tokio::sync::{Mutex, MutexGuard};

use crate::scheduler::abort_scheduled_task;

type STORAGE = HashMap<String, Box<dyn Any + Send + Sync>>;
lazy_static::lazy_static! {
    static ref CACHE_STORE: Mutex<STORAGE> =
        Mutex::new(HashMap::new());
}

pub async fn get_cache_connection() -> MutexGuard<'static, STORAGE> {
    CACHE_STORE.lock().await
}

const TOURNAMENT_PREFIX: &str = "T-";
const TYPING_SESSION_PREFIX: &str = "TS-";

pub async fn cache_get_tournament(tournament_id: &str) -> Option<TournamentSession> {
    let conn = get_cache_connection().await;
    let value = conn.get(&generate_tournament_cache_id(tournament_id));
    value
        .map(|v| v.downcast_ref::<TournamentSession>())
        .flatten()
        .map(|v| v.clone())
}

pub async fn cache_set_tournament(tournament_id: &str, tournament: TournamentSession) {
    let mut conn = get_cache_connection().await;
    conn.insert(
        generate_tournament_cache_id(tournament_id),
        Box::new(tournament),
    );
}

pub async fn cache_update_tournament(
    tournament_id: &str,
    update: impl FnOnce(&mut TournamentSession),
) {
    let mut conn = get_cache_connection().await;
    if let Some(tournament) = conn.get_mut(&generate_tournament_cache_id(tournament_id)) {
        let tournament = tournament.downcast_mut::<TournamentSession>().unwrap();
        update(tournament);
        if tournament.joined == 0 {
            conn.remove(&generate_tournament_cache_id(tournament_id));
            abort_scheduled_task(&tournament_id.to_owned()).await.ok(); //
        }
    }
}

pub async fn cache_get_typing_session(client_id: &str) -> Option<TypingSessionSchema> {
    let conn = get_cache_connection().await;
    let value = conn
        .values()
        .map(|v| v.downcast_ref::<TypingSessionSchema>())
        .find(|v| matches!(v, Some(v) if v.client.client_id == client_id))
        .flatten();
    value.map(|v| v.clone())
}

pub async fn cache_set_typing_session(session: TypingSessionSchema) {
    let mut conn = get_cache_connection().await;
    let cache_id =
        generate_typing_session_cache_id(&session.tournament_id, &session.client.client_id);
    conn.insert(cache_id, Box::new(session));
}

pub async fn cache_delete_typing_session(tournament_id: &str, client_id: &str) {
    let mut conn = get_cache_connection().await;
    conn.remove(&generate_typing_session_cache_id(tournament_id, client_id));
    cache_update_tournament(tournament_id, |t| t.current -= 1).await;
}

fn generate_typing_session_cache_id(tournament_id: &str, client_id: &str) -> String {
    format!("{TYPING_SESSION_PREFIX}-{}-{}", tournament_id, client_id)
}

fn generate_tournament_cache_id(tournament_id: &str) -> String {
    format!("{TOURNAMENT_PREFIX}-{}", tournament_id)
}
