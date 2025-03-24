use utils::testing::setup_test_db;

mod tournament;
mod user;

use tournament::test_tournament;
use user::test_user;

#[tokio::test]
async fn user_main() {
    let db = setup_test_db("sqlite::memory:")
        .await
        .expect("Set up db failed!");

    test_user(&db).await;
}

#[tokio::test]
async fn tournament_main() {
    let db = setup_test_db("sqlite::memory:")
        .await
        .expect("Set up db failed!");

    test_user(&db).await;
    test_tournament(&db).await;
}
