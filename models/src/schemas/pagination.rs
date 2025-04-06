use serde::Serialize;

#[derive(Serialize)]
pub struct ListSchema<T> {
    pub data: Vec<T>,
}

impl<U, T: From<U>> From<Vec<U>> for ListSchema<T> {
    fn from(data: Vec<U>) -> Self {
        Self {
            data: data.into_iter().map(T::from).collect(),
        }
    }
}

#[derive(Serialize)]
pub struct PaginatedData<T> {
    pub data: Vec<T>,
    pub page: u64,
    pub limit: u64,
    pub total: u64,
}

impl<T> PaginatedData<T> {
    pub const fn new(data: Vec<T>, page: u64, limit: u64, total: u64) -> Self {
        Self {
            data,
            page,
            limit,
            total,
        }
    }
}
