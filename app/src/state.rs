use sea_orm::DatabaseConnection;

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub conn: DatabaseConnection,
    pub config: Config,
}
