use std::collections::HashMap;

use serde::Serialize;

#[derive(Serialize)]
pub struct ApiErrorResponse {
    pub message: String,
}

#[derive(Serialize)]
pub struct ValidationErrorResponse<T> {
    pub message: String,
    pub details: T,
}

impl<T> From<T> for ValidationErrorResponse<T> {
    fn from(t: T) -> Self {
        Self {
            message: "Validation error".to_string(),
            details: t,
        }
    }
}
