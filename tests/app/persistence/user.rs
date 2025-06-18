use sea_orm::DatabaseConnection;

use app::persistence::users::{create_user, get_user};
use models::params::user::CreateUserParams;

pub(super) async fn test_user(db: &DatabaseConnection) {
    let params = CreateUserParams {
        password: "".to_string(),
        email: "".to_string(),
    };

    let user = create_user(db, params).await.expect("Create user failed!");
    let expected = get_user(db, &user.id)
        .await
        .expect("Get user failed!")
        .expect("User not found");

    assert_eq!(user, expected);
}
