use sea_orm::{DbConn, DbErr, EntityTrait};

use models::domains::{texts, tournaments};

pub async fn get_or_generate_text(db: &DbConn, tournament_id: String) -> Result<String, DbErr> {
    let tournament = tournaments::Entity::find_by_id(tournament_id.clone())
        .one(db)
        .await
        .unwrap();

    let text_id = tournament.map(|t| t.text_id).flatten();

    if let Some(text_id) = text_id {
        let text = texts::Entity::find_by_id(text_id).one(db).await.unwrap();

        if let Some(text) = text {
            return Ok(text.content);
        }
    } else {
        return Ok("In the land of myth and in the time of magic, the destiny of a great kingdom rests on the shoulders of a young boy. He's named Merlin.".to_string());
    }

    return Ok("In the land of myth and in the time of magic, the destiny of a great kingdom rests on the shoulders of a young boy. He's named Merlin.".to_string());
}
