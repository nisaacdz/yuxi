use app::{
    cache::{TournamentRegistry, TypingSessionRegistry},
    config::Config,
    state::AppState,
};
use socketioxide::SocketIo;
use tokio::sync::OnceCell;
use utils::testing::setup_test_db;

mod tournament;
mod user;

use tournament::test_tournament;
use user::test_user;

static APP_STATE: OnceCell<AppState> = OnceCell::const_new();

async fn get_app_state() -> &'static AppState {
    if let Some(state) = APP_STATE.get() {
        return state;
    }
    APP_STATE
        .get_or_init(async move || {
            let config = Config::from_env().await;
            let conn = setup_test_db("sqlite::memory:")
                .await
                .expect("Set up db failed!");

            let (_, socket_io) = SocketIo::new_layer();

            AppState {
                conn,
                config,
                tournament_registry: TournamentRegistry::new(),
                typing_session_registry: TypingSessionRegistry::new(),
                socket_io,
            }
        })
        .await
}

#[tokio::test]
async fn user_main() {
    let app_state = get_app_state().await;

    test_user(app_state).await;
}

#[tokio::test]
async fn tournament_main() {
    let app_state = get_app_state().await;

    test_user(app_state).await;
    test_tournament(app_state).await;
}
