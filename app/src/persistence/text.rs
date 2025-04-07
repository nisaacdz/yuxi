use fake::Fake;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DbConn, DbErr, EntityTrait};

use models::domains::{texts, tournaments};

pub async fn get_or_generate_text(db: &DbConn, tournament_id: &str) -> Result<String, DbErr> {
    let tournament = tournaments::Entity::find_by_id(tournament_id.to_owned())
        .one(db)
        .await?;

    let text_id = tournament.as_ref().map(|t| t.text_id).flatten();

    let text_from_id = if let Some(text_id) = text_id {
        texts::Entity::find_by_id(text_id)
            .one(db)
            .await?
            .map(|t| t.content)
    } else {
        None
    };

    let text = if let Some(text_from_id) = text_from_id {
        text_from_id
    } else {
        let text_options = tournament
            .as_ref()
            .map(|t| t.text_options.clone())
            .flatten();
        let new: String = fake::faker::lorem::en::Paragraph(3..5).fake();
        models::domains::texts::ActiveModel {
            content: Set(new.clone()),
            options: Set(text_options),
            ..Default::default()
        }
        .save(db)
        .await?;
        new
    };

    return Ok(text);
}
