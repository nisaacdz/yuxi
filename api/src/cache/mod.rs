use std::{collections::HashMap, sync::{Arc, Mutex, MutexGuard}};
#[derive(Clone)]
pub struct Cache<T> {
    data: Arc<Mutex<HashMap<String, T>>>,
}

impl<T> Cache<T> {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    fn get_connection(&self) -> MutexGuard<'_, HashMap<String, T>> {
        self.data.lock().unwrap()
    }
    pub fn set_data(&self, id: &str, data: T) {
        let mut conn = self.get_connection();
        conn.insert(id.to_owned(), data);
    }

    pub fn update_data<F, O>(&self, id: &str, update: F) -> Option<O>
    where F: FnOnce(&mut T) -> O
    {
        let mut conn = self.get_connection();
        conn.get_mut(id).map(|data| update(data))
    }

    pub fn delete_data(&self, id: &str) {
        let mut conn = self.get_connection();
        conn.remove(id);
    }
}

impl<T: Clone> Cache<T> {
    pub fn get_data(&self, id: &str) -> Option<T> {
        let conn = self.get_connection();
        conn.get(id)
            .map(|data| data.clone())
    }
}

impl<T> Default for Cache<T> {
    fn default() -> Self {
        Self::new()
    }
}
