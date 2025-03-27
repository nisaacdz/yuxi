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
    last_call: Instant,
    stack_size: u32,
    handle: Option<JoinHandle<()>>,
}

pub struct Moderate<F>(pub F);

impl<F> Moderate<F>
where
    F: Future<Output = ()> + Send + 'static,
{
    pub async fn with_key(self, key: &str) {
        let key = key.to_owned();
        let fut = self.0;

        // 1. Brief outer lock to get entry reference
        let entry_arc = {
            let mut state = STATE.lock().await;
            state
                .entry(key.clone())
                .or_insert_with(|| {
                    Arc::new(Mutex::new(CallInfo {
                        last_call: Instant::now(),
                        stack_size: 0,
                        handle: None,
                    }))
                })
                .clone()
        };

        // 2. Work with the entry without holding outer lock
        let mut entry = entry_arc.lock().await;
        entry.stack_size += 1;
        entry.last_call = Instant::now();

        // Immediate execution if burst threshold reached
        if entry.stack_size >= MAX_STACK_SIZE {
            entry.stack_size = 0;
            if let Some(handle) = entry.handle.take() {
                handle.abort();
            }
            tokio::spawn(fut);
            Self::schedule_cleanup(key).await;
            return;
        }

        // Cancel previous task if exists
        if let Some(handle) = entry.handle.take() {
            handle.abort();
        }

        // 3. Clone Arc for async task
        let entry_arc_clone = entry_arc.clone();
        entry.handle = Some(tokio::spawn(async move {
            sleep(Duration::from_millis(TIMEOUT_DURATION)).await;

            // Execute if still valid after timeout
            let entry = entry_arc_clone.lock().await;
            if entry.stack_size > 0 {
                let fut = fut;
                tokio::spawn(fut);
            }

            Self::schedule_cleanup(key).await;
        }));
    }

    async fn schedule_cleanup(key: String) {
        // Brief outer lock to remove entry if unused
        let mut state = STATE.lock().await;
        if let Some(existing) = state.get(&key) {
            if Arc::strong_count(existing) == 1 {
                state.remove(&key);
            }
        }
    }
}
