use app::{persistence::users::{create_user, get_user}, state::AppState};
use models::params::user::CreateUserParams;

pub(super) async fn test_user(state: &AppState) {
    let params = CreateUserParams {
        password: "".to_string(),
        email: "".to_string(),
    };

    let user = create_user(state, params).await.expect("Create user failed!");
    let expected = get_user(state, &user.id)
        .await
        .expect("Get user failed!")
        .expect("User not found");

    assert_eq!(user, expected);
}
