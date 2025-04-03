use crate::domains::texts;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct TextSchema {
    pub id: i32,
    pub content: String,
    pub options: Option<TextOptions>,
}

impl From<texts::Model> for TextSchema {
    fn from(text: texts::Model) -> Self {
        Self {
            id: text.id,
            content: text.content,
            options: text
                .options
                .map(|v| serde_json::from_value(v).unwrap_or_default()),
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TextOptions {
    upper_case: bool,
    lower_case: bool,
    numbers: bool,
    symbols: bool,
    meaningful_words: bool,
}

impl Default for TextOptions {
    fn default() -> Self {
        Self {
            upper_case: true,
            lower_case: true,
            numbers: true,
            symbols: true,
            meaningful_words: true,
        }
    }
}
