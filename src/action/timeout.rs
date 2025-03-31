use std::{
    future::Future,
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::{sync::Mutex, task::JoinHandle, time::sleep};

const CLEANUP_WAIT_DURATION: u64 = 30;

struct Inner<C> {
    last_executed: Option<Instant>,
    cleanup: C,
    cleanup_handle: Option<JoinHandle<()>>,
}

pub struct TimeoutMonitor<C> {
    inner: Arc<Mutex<Inner<C>>>,
}

impl<Fut, C> TimeoutMonitor<C>
where
    Fut: Future<Output = ()> + Send + Sync + 'static,
    C: Fn() -> Fut + Send + Sync + 'static,
{
    pub fn new(cleanup: C) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                last_executed: None,
                cleanup,
                cleanup_handle: None,
            })),
        }
    }

    pub async fn call<F: Future<Output = ()> + Send + Sync + 'static>(&self, fut: F) {
        let mut inner_lock = self.inner.lock().await;
        if let Some(cleanup_handle) = inner_lock.cleanup_handle.take() {
            cleanup_handle.abort();
        }
        std::mem::drop(inner_lock);
        let inner = self.inner.clone();
        tokio::spawn(async move {
            fut.await;
            let inner_clone = inner.clone();
            let mut lock_again = inner.lock().await;
            lock_again.last_executed = Some(Instant::now());
            lock_again.cleanup_handle = Some(tokio::spawn(async move {
                sleep(Duration::from_secs(CLEANUP_WAIT_DURATION)).await;
                let mut lock_again = inner_clone.lock().await;
                (lock_again.cleanup)().await;
                lock_again.last_executed = None;
                std::mem::drop(lock_again);
            }));
        });
    }
}
