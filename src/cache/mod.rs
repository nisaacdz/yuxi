use models::schemas::{tournament::TournamentInfo, typing::TypingSession};
use redis::{aio::MultiplexedConnection, AsyncCommands, Client};
use tokio::sync::OnceCell;

static REDIS_CLIENT: OnceCell<Client> = OnceCell::const_new();

pub async fn get_redis_connection() -> MultiplexedConnection {
    let client = REDIS_CLIENT.get().expect("Redis client not initialized");
    client
        .get_multiplexed_tokio_connection()
        .await
        .expect("Redis connection failed")
}

pub async fn redis_get_tournament(tournament_id: &str) -> Option<TournamentInfo> {
    let mut conn = get_redis_connection().await;
    let value: Option<String> = conn
        .get(format!("T-{}", tournament_id))
        .await
        .expect("Redis query failed");
    value.map(|v| serde_json::from_str(&v).expect("Deserialization failed"))
}

pub async fn redis_set_tournament(tournament_id: &str, tournament: TournamentInfo) {
    let mut conn = get_redis_connection().await;
    let tournament_json: String = serde_json::to_string(&tournament).expect("Serialization failed");
    let _: () = conn
        .set(format!("T-{}", tournament_id), tournament_json)
        .await
        .expect("Redis query failed");
}

pub async fn redis_update_tournament(
    tournament_id: &str,
    update: fn(TournamentInfo) -> TournamentInfo,
) {
    let mut conn = get_redis_connection().await;
    let value: Option<String> = conn
        .get(format!("T-{}", tournament_id))
        .await
        .expect("Redis query failed");
    let tournament: TournamentInfo = value
        .map(|v| serde_json::from_str(&v).expect("Deserialization failed"))
        .expect("Tournament not found");
    let updated_tournament = update(tournament);
    let tournament_json: String =
        serde_json::to_string(&updated_tournament).expect("Serialization failed");
    let _: () = conn
        .set(format!("T-{}", tournament_id), tournament_json)
        .await
        .expect("Redis query failed");
}

pub async fn redis_delete_tournament(tournament_id: &str) {
    let mut conn = get_redis_connection().await;
    let _: () = conn
        .del(format!("T-{}", tournament_id))
        .await
        .expect("Redis query failed");
}

pub async fn redis_get_typing_session(client_id: &str) -> Option<TypingSession> {
    let mut conn = get_redis_connection().await;
    let value: Option<String> = conn
        .get(format!("TS-{}", client_id))
        .await
        .expect("Redis query failed");
    value.map(|v| serde_json::from_str(&v).expect("Deserialization failed"))
}

pub async fn redis_set_typing_session(client_id: &str, session: TypingSession) {
    let mut conn = get_redis_connection().await;
    let session_json: String = serde_json::to_string(&session).expect("Serialization failed");
    let _: () = conn
        .set(format!("TS-{}", client_id), session_json)
        .await
        .expect("Redis query failed");
}

pub async fn redis_delete_typing_session(tournament_id: &str, client_id: &str) {
    let mut conn = get_redis_connection().await;
    let _: () = conn
        .del(format!("TS-{}-{}", tournament_id, client_id))
        .await
        .expect("Redis query failed");
}

pub fn initialize_redis(redis_url: &String) {
    let client = Client::open(redis_url.to_owned()).expect("Redis connection failed");
    REDIS_CLIENT.set(client).unwrap();
}
