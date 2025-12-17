use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
};

use models::schemas::typing::TypingSessionSchema;

use crate::core::TournamentManager;

// Actually, it might be too memory consuming -- from sparse/unrelated keys
// mod trie {
//     use super::*;
//     //use super::super::persistence::ID_ALPHABET; 0-9, A-Z, a-z
//     // TODO: update the dict to use 62 length and calculated indexing

//     struct TrieNodeInner<T> {
//         value: Option<T>,
//         dict: Box<[Option<TrieNode<T>>; 128]>,
//     }

//     impl<T> TrieNodeInner<T> {
//         fn new(value: Option<T>) -> Self {
//             let nodes = const { [const { None }; 128] };

//             Self {
//                 value,
//                 dict: Box::new(nodes),
//             }
//         }
//     }

//     struct TrieNode<T> {
//         inner: Arc<Mutex<TrieNodeInner<T>>>,
//     }

//     impl<T> Default for TrieNode<T> {
//         fn default() -> Self {
//             Self {
//                 inner: Arc::new(Mutex::new(TrieNodeInner::new(None))),
//             }
//         }
//     }

//     impl<T> Clone for TrieNode<T> {
//         fn clone(&self) -> Self {
//             Self {
//                 inner: self.inner.clone(),
//             }
//         }
//     }

//     impl<T> TrieNode<T> {
//         fn insert(&self, key: &[u8], value: T) {
//             let mut inner = self.inner.lock().unwrap();
//             if key.is_empty() {
//                 inner.value.replace(value);
//                 return;
//             }
//             let node = inner.dict[key[0] as usize].get_or_insert_default().clone();
//             std::mem::drop(inner);
//             return node.insert(&key[1..], value);
//         }

//         fn get(&self, key: &[u8]) -> Option<T>
//         where
//             T: Clone,
//         {
//             let inner = self.inner.lock().unwrap();
//             if key.is_empty() {
//                 return inner.value.clone();
//             }
//             if let Some(node) = inner.dict[key[0] as usize].clone() {
//                 std::mem::drop(inner);
//                 return node.get(&key[1..]);
//             } else {
//                 return None;
//             }
//         }

//         fn get_or_insert<F>(&self, key: &[u8], with: F) -> T
//         where
//             F: FnOnce() -> T,
//             T: Clone,
//         {
//             let mut inner = self.inner.lock().unwrap();
//             if key.is_empty() {
//                 return inner.value.get_or_insert_with(with).clone();
//             }

//             let node = inner.dict[key[0] as usize].get_or_insert_default().clone();
//             std::mem::drop(inner);
//             return node.get_or_insert(&key[1..], with);
//         }

//         fn contains(&self, key: &[u8]) -> bool {
//             let inner = self.inner.lock().unwrap();
//             if key.is_empty() {
//                 return inner.value.is_some();
//             }
//             if let Some(node) = inner.dict[key[0] as usize].clone() {
//                 std::mem::drop(inner);
//                 return node.contains(&key[1..]);
//             } else {
//                 return false;
//             }
//         }

//         fn remove(&self, key: &[u8]) -> Option<T> {
//             let mut inner = self.inner.lock().unwrap();
//             if key.is_empty() {
//                 return inner.value.take();
//             }
//             if let Some(node) = inner.dict[key[0] as usize].clone() {
//                 std::mem::drop(inner);
//                 return node.remove(&key[1..]);
//             } else {
//                 return None;
//             }
//         }

//         fn update<F, O>(&self, key: &[u8], update: F) -> Option<O>
//         where
//             F: FnOnce(&mut T) -> O,
//         {
//             let mut inner = self.inner.lock().unwrap();
//             if key.is_empty() {
//                 return inner.value.as_mut().map(update);
//             }
//             if let Some(node) = inner.dict[key[0] as usize].clone() {
//                 std::mem::drop(inner);
//                 return node.update(&key[1..], update);
//             } else {
//                 return None;
//             }
//         }
//     }

//     pub struct TrieCache<T> {
//         root: TrieNode<T>,
//     }

//     impl<T> TrieCache<T> {
//         pub fn new() -> Self {
//             Self {
//                 root: TrieNode::default(),
//             }
//         }

//         pub fn set_data(&self, id: &str, data: T) {
//             self.root.insert(id.as_bytes(), data);
//         }

//         pub fn contains_key(&self, id: &str) -> bool {
//             self.root.contains(id.as_bytes())
//         }

//         pub fn update_data<F, O>(&self, id: &str, update: F) -> Option<O>
//         where
//             F: FnOnce(&mut T) -> O,
//         {
//             self.root.update(id.as_bytes(), update)
//         }

//         pub fn delete_data(&self, id: &str) -> Option<T> {
//             self.root.remove(id.as_bytes())
//         }

//         pub fn values(&self) -> Vec<T>
//         where
//             T: Clone,
//         {
//             // rewrite to return iterator instead
//             todo!()
//         }

//         pub fn keys(&self) -> Vec<String> {
//             // rewrite to return iterator instead
//             todo!()
//         }

//         pub fn count(&self) -> usize {
//             // store length on each node (of the subtrees) --- or just alongside the root
//             0
//         }
//     }

//     impl<T: Clone> TrieCache<T> {
//         pub fn get_data(&self, id: &str) -> Option<T> {
//             self.root.get(id.as_bytes())
//         }

//         pub fn get_or_insert<F>(&self, id: &str, with: F) -> T
//         where
//             F: FnOnce() -> T,
//         {
//             self.root.get_or_insert(id.as_bytes(), with)
//         }
//     }

//     impl<T> Default for TrieCache<T> {
//         fn default() -> Self {
//             Self::new()
//         }
//     }
// }

pub struct Cache<T> {
    data: Arc<Mutex<HashMap<String, T>>>,
}

impl<T> Clone for Cache<T> {
    fn clone(&self) -> Self {
        Cache {
            data: Arc::clone(&self.data),
        }
    }
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

    pub fn contains_key(&self, id: &str) -> bool {
        let conn = self.get_connection();
        conn.contains_key(id)
    }

    pub fn update_data<F, O>(&self, id: &str, update: F) -> Option<O>
    where
        F: FnOnce(&mut T) -> O,
    {
        let mut conn = self.get_connection();
        conn.get_mut(id).map(|data| update(data))
    }

    pub fn read_data<F, O>(&self, id: &str, read: F) -> Option<O>
    where
        F: FnOnce(&T) -> O,
    {
        let conn = self.get_connection();
        conn.get(id).map(|data| read(data))
    }

    pub fn delete_data(&self, id: &str) -> Option<T> {
        let mut conn = self.get_connection();
        conn.remove(id)
    }

    pub fn values(&self) -> Vec<T>
    where
        T: Clone,
    {
        let conn = self.get_connection();
        conn.values().cloned().collect()
    }

    pub fn keys(&self) -> Vec<String> {
        let conn = self.get_connection();
        conn.keys().cloned().collect()
    }

    pub fn count(&self) -> usize {
        self.get_connection().len()
    }
}

impl<T: Clone> Cache<T> {
    pub fn get_data(&self, id: &str) -> Option<T> {
        let conn = self.get_connection();
        conn.get(id).map(|data| data.clone())
    }

    pub fn get_or_insert<F>(&self, id: &str, with: F) -> T
    where
        F: FnOnce() -> T,
    {
        let mut conn = self.get_connection();
        conn.entry(id.to_owned()).or_insert_with(with).clone()
    }
}

impl<T> Default for Cache<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct TournamentRegistry {
    registry: Cache<TournamentManager>,
}

impl TournamentRegistry {
    pub fn new() -> Self {
        Self {
            registry: Cache::new(),
        }
    }

    pub fn get(&self, id: &str) -> Option<TournamentManager> {
        self.registry.get_data(id)
    }

    pub fn get_or_init<F>(&self, tournament_id: String, with: F) -> TournamentManager
    where
        F: FnOnce() -> TournamentManager,
    {
        self.registry.get_or_insert(&tournament_id, || with())
    }

    pub fn evict(&self, tournament_id: &str) -> Option<TournamentManager> {
        self.registry.delete_data(tournament_id)
    }
}

#[derive(Clone)]
pub struct TypingSessionRegistry {
    sessions: Cache<TypingSessionSchema>,
}

impl TypingSessionRegistry {
    pub fn new() -> Self {
        Self {
            sessions: Cache::new(),
        }
    }

    pub fn contains_session(&self, id: &str) -> bool {
        self.sessions.contains_key(id)
    }

    pub fn get_session(&self, id: &str) -> Option<TypingSessionSchema> {
        self.sessions.get_data(id)
    }

    pub fn set_session(&self, id: &str, session: TypingSessionSchema) {
        self.sessions.set_data(id, session);
    }

    pub fn delete_session(&self, id: &str) -> Option<TypingSessionSchema> {
        self.sessions.delete_data(id)
    }
}
