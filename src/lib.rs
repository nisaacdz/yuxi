#[cfg(feature = "shuttle")]
pub mod shuttle;
#[cfg(not(feature = "shuttle"))]
pub mod tokio;

pub(crate) mod middleware;

#[derive(Clone, Debug)]
pub struct UserSession {
    pub client_id: String,
    pub user_id: Option<String>,
}
