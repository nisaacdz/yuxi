use sea_orm::prelude::DateTime;
use sea_orm::{DatabaseConnection, Unchanged};

use app::persistence::tournaments::create_tournament;
use models::domains::tournaments;
use models::params::tournament::CreateTournamentParams;

pub(super) async fn test_tournament(db: &DatabaseConnection) {
    let params = CreateTournamentParams {
        title: "title".to_string(),
        scheduled_for: "2021-01-01 00:00:00".parse().unwrap(),
    };

    let tournament = create_tournament(db, params)
        .await
        .expect("Create tournament failed!");
    let expected = tournaments::ActiveModel {
        title: Unchanged("title".to_owned()),
        scheduled_for: Unchanged(
            DateTime::parse_from_str("2021-01-01 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap(),
        ),
        ..Default::default()
    };
    assert_eq!(tournament, expected);
}
