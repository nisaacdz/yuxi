use crate::domains::texts;
use serde::Serialize;

#[derive(Serialize)]
pub struct TextSchema {
    pub id: i32,
    pub content: String,
}

impl From<texts::Model> for TextSchema {
    fn from(text: texts::Model) -> Self {
        Self {
            id: text.id,
            content: text.content,
        }
    }
}

#[derive(Serialize)]
pub struct TextListSchema {
    pub texts: Vec<TextSchema>,
}

impl From<Vec<texts::Model>> for TextListSchema {
    fn from(texts: Vec<texts::Model>) -> Self {
        Self {
            texts: texts.into_iter().map(TextSchema::from).collect(),
        }
    }
}
