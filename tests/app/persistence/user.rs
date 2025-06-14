use sea_orm::{DatabaseConnection, Unchanged};

use app::persistence::users::create_user;
use models::domains::users;
use models::params::user::CreateUserParams;

pub(super) async fn test_user(db: &DatabaseConnection) {
    let params = CreateUserParams {
        password: "".to_string(),
        email: "".to_string(),
    };

    let user = create_user(db, params).await.expect("Create user failed!");
    let expected = users::ActiveModel {
        username: Unchanged("test".to_owned()),
        passhash: Unchanged("".to_owned()),
        email: Unchanged("".to_owned()),
        ..Default::default()
    };
    assert_eq!(user, expected);
}
