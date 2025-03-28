use lazy_static::lazy_static;
use std::{
    collections::HashMap,
    future::Future,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{sync::Mutex, task::JoinHandle, time::sleep};

const TIMEOUT_DURATION: u64 = 100;
const MAX_STACK_SIZE: u32 = 20;

lazy_static! {
    static ref STATE: Mutex<HashMap<String, Arc<Mutex<CallInfo>>>> = Mutex::new(HashMap::new());
}

struct CallInfo {
    typed_chars: Vec<char>,
    last_call: Instant,
    stack_size: u32,
    handle: Option<JoinHandle<()>>,
}

pub struct TypingModerator<F>(pub F);

impl<F, Fut> TypingModerator<F>
where
    F: FnOnce(Vec<char>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    pub async fn moderate(self, key: &str, character: char) {
        let key = key.to_owned();
        let processor = self.0;

        let entry_arc = {
            let mut state = STATE.lock().await;
            state
                .entry(key.clone())
                .or_insert_with(|| {
                    Arc::new(Mutex::new(CallInfo {
                        typed_chars: Vec::new(),
                        last_call: Instant::now(),
                        stack_size: 0,
                        handle: None,
                    }))
                })
                .clone()
        };

        let mut entry = entry_arc.lock().await;
        entry.typed_chars.push(character);
        entry.stack_size += 1;
        entry.last_call = Instant::now();

        // Capture current batch and reset immediately
        let should_process = entry.stack_size >= MAX_STACK_SIZE;
        let current_batch = if should_process {
            let batch = std::mem::replace(&mut entry.typed_chars, Vec::new());
            entry.stack_size = 0;
            Some(batch)
        } else {
            None
        };

        if let Some(batch) = current_batch {
            if let Some(handle) = entry.handle.take() {
                handle.abort();
            }
            tokio::spawn(async move {
                processor(batch).await;
                Self::schedule_cleanup(key).await;
            });
            return;
        }

        if let Some(handle) = entry.handle.take() {
            handle.abort();
        }

        let entry_arc_clone = entry_arc.clone();
        let key_clone = key.clone();
        entry.handle = Some(tokio::spawn(async move {
            sleep(Duration::from_millis(TIMEOUT_DURATION)).await;

            // Take ownership of the current characters
            let batch = {
                let mut entry = entry_arc_clone.lock().await;
                let batch = std::mem::replace(&mut entry.typed_chars, Vec::new());
                entry.stack_size = 0;
                batch
            };

            if !batch.is_empty() {
                processor(batch).await;
            }

            Self::schedule_cleanup(key_clone).await;
        }));
    }

    async fn schedule_cleanup(key: String) {
        let mut state = STATE.lock().await;
        if let Some(existing) = state.get(&key) {
            if Arc::strong_count(existing) == 1 {
                state.remove(&key);
            }
        }
    }
}
