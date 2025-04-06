use app::persistence::users::create_user;
use models::params::user::CreateUserParams;
use models::schemas::user::UserSchema;
use sea_orm::{DatabaseConnection, TryIntoModel, Unchanged};

use app::persistence::tournaments::create_tournament;
use models::domains::tournaments;
use models::params::tournament::CreateTournamentParams;

pub(super) async fn test_tournament(db: &DatabaseConnection) {
    let create_user_params = CreateUserParams {
        email: "username".to_string(),
        password: "password".to_string(),
    };

    let user = UserSchema::from(
        create_user(db, create_user_params)
            .await
            .unwrap()
            .try_into_model()
            .unwrap(),
    );

    let create_tournament_params = CreateTournamentParams {
        title: "title".to_string(),
        scheduled_for: "2021-01-01 00:00:00".parse().unwrap(),
    };

    let tournament = create_tournament(db, create_tournament_params, &user)
        .await
        .expect("Create tournament failed!");
    let expected = tournaments::ActiveModel {
        title: Unchanged("title".to_owned()),
        scheduled_for: Unchanged("2021-01-01 00:00:00".parse().unwrap()),
        ..Default::default()
    };
    assert_eq!(tournament.id, expected.id);
}
