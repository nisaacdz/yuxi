use api::{setup_db, setup_router};
use utils::migrate;

pub async fn run(db_url: &str) -> shuttle_axum::ShuttleAxum {
    tracing::info!("Starting with shuttle");

    let conn = setup_db(&db_url).await;
    migrate(&conn).await.expect("Migration failed!");

    let router = setup_router(conn);
    Ok(router.into())
}
