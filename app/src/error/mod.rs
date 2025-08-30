use axum::http::StatusCode;

#[derive(Debug)]
pub struct CustomError {
    pub code: StatusCode,
    pub message: String,
}

impl std::fmt::Display for CustomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.message)
    }
}

impl std::error::Error for CustomError {}

impl CustomError {
    pub fn new(code: StatusCode, message: String) -> Self {
        Self { code, message }
    }
}
