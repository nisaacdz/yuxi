use serde::Deserialize;

pub mod user;

#[derive(Deserialize)]
pub struct PaginationQuery {
    pub page: Option<u64>,
    pub limit: Option<u64>,
    pub sort: Option<String>,
    pub filter: Option<String>,
    pub search: Option<String>,
}

impl Default for PaginationQuery {
    fn default() -> Self {
        Self {
            page: Some(1),
            limit: Some(15),
            sort: None,
            filter: None,
            search: None,
        }
    }
}
