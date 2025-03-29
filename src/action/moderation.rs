use std::{
    future::Future,
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::{sync::Mutex, task::JoinHandle};

const MAX_DEBOUNCE_WAIT: u64 = 100;
const MAX_STACK_SIZE: u32 = 20;

pub struct Inner {
    typed_chars: Vec<char>,
    last_call: Option<Instant>,
    stack_size: u32,
    handle: Option<JoinHandle<()>>,
}

pub struct FrequencyMonitor {
    inner: Arc<Mutex<Inner>>,
}

impl FrequencyMonitor {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                typed_chars: Vec::new(),
                last_call: None,
                stack_size: 0,
                handle: None,
            })),
        }
    }

    pub async fn call<Fut, F>(&self, c: char, processor: F)
    where
        Fut: Send + Future<Output = ()>,
        F: Fn(Vec<char>) -> Fut + Send + Sync + 'static,
    {
        let mut inner_lock = self.inner.lock().await;
        let now = Instant::now();
        if let Some(handle) = inner_lock.handle.take() {
            handle.abort();
        }
        inner_lock.typed_chars.push(c);
        inner_lock.stack_size += 1;
        inner_lock.last_call = Some(now);

        if inner_lock.stack_size > MAX_STACK_SIZE {
            let typed_chars = std::mem::replace(&mut inner_lock.typed_chars, Vec::new());
            inner_lock.stack_size = 0;
            let handle = tokio::spawn(async move { processor(typed_chars).await });

            inner_lock.handle = Some(handle);
        } else {
            let inner = self.inner.clone();
            let handle = tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(MAX_DEBOUNCE_WAIT)).await;

                let mut inner_lock = inner.lock().await;

                let typed_chars = std::mem::replace(&mut inner_lock.typed_chars, Vec::new());

                inner_lock.stack_size = 0;
                processor(typed_chars).await;
            });
            inner_lock.handle = Some(handle);
        }
    }
}
