mod action;
mod error;
mod extractor;
mod init;
mod middleware;
mod validation;

pub mod models;
pub mod routers;

pub use init::{setup_config, setup_db, setup_router};
use serde::Serialize;

/// A generic structure for API responses sent over WebSockets.
#[derive(Serialize, Debug)]
pub struct ApiResponse<T: Serialize> {
    success: bool,
    message: String,
    data: Option<T>,
}

impl<T: Serialize> ApiResponse<T> {
    /// Creates a successful API response.
    ///
    /// # Arguments
    ///
    /// * `message` - A descriptive success message.
    /// * `data` - Optional data payload associated with the success.
    pub fn success(message: &str, data: Option<T>) -> Self {
        Self {
            success: true,
            message: message.to_string(),
            data,
        }
    }

    /// Creates an error API response.
    ///
    /// # Arguments
    ///
    /// * `message` - A descriptive error message.
    pub fn error(message: &str) -> Self {
        Self {
            success: false,
            message: message.to_string(),
            data: None,
        }
    }

    /// Checks if the response indicates success.
    pub fn is_success(&self) -> bool {
        self.success
    }

    /// Consumes the response and returns the inner data if successful.
    /// Returns `None` if the response was an error or had no data.
    pub fn into_data(self) -> Option<T> {
        if self.success { self.data } else { None }
    }
}
