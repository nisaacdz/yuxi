// moderation.rs
use std::{future::Future, sync::Arc, time::Duration};
use tokio::{sync::Mutex, task::JoinHandle};

const MAX_DEBOUNCE_WAIT: u64 = 100; // millis
const MAX_STACK_SIZE: u32 = 20;

struct Inner<F> {
    buffer: Vec<char>,
    stack_size: u32,
    processor: F,
    handle: Option<JoinHandle<()>>,
}

pub struct FrequencyMonitor<F> {
    inner: Arc<Mutex<Inner<F>>>,
}

impl<F, Fut> FrequencyMonitor<F>
where
    F: Fn(Vec<char>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    pub fn new(processor: F) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                buffer: Vec::new(),
                stack_size: 0,
                processor,
                handle: None,
            })),
        }
    }

    pub async fn push(&self, c: char) {
        let mut inner = self.inner.lock().await;
        inner.buffer.push(c);
        inner.stack_size += 1;

        if inner.stack_size >= MAX_STACK_SIZE {
            self.flush(&mut inner).await;
        } else {
            self.schedule_flush(&mut inner).await;
        }
    }

    async fn flush(&self, inner: &mut Inner<F>) {
        if let Some(handle) = inner.handle.take() {
            handle.abort();
        }
        
        let buffer = std::mem::replace(&mut inner.buffer, Vec::new());
        inner.stack_size = 0;
        
        let processor = &inner.processor;
        tokio::spawn(processor(buffer));
    }

    async fn schedule_flush(&self, inner: &mut Inner<F>) {
        if let Some(handle) = inner.handle.take() {
            handle.abort();
        }

        let inner_clone = self.inner.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(MAX_DEBOUNCE_WAIT)).await;
            
            let mut inner = inner_clone.lock().await;
            let buffer = std::mem::replace(&mut inner.buffer, Vec::new());
            inner.stack_size = 0;
            
            let processor = &inner.processor;
            processor(buffer).await;
        });

        inner.handle = Some(handle);
    }
}