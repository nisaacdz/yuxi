use std::{
    future::Future,
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::{sync::Mutex, task::JoinHandle, time::sleep};

const CLEANUP_WAIT_DURATION: u64 = 60 * 30; // millis

struct Inner<C> {
    last_call: Option<Instant>,
    cleanup: C,
    cleanup_handle: Option<JoinHandle<()>>,
}

pub struct TimeoutMonitor<C> {
    inner: Arc<Mutex<Inner<C>>>,
}

impl<Fut: Future<Output = ()> + Send + Sync + 'static, C: Fn() -> Fut> TimeoutMonitor<C> {
    pub fn new(cleanup: C) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                last_call: None,
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
        inner_lock.last_call = Some(Instant::now());
        tokio::spawn(fut);
        let clean_up_fut = (inner_lock.cleanup)(); // thank god rust guarantees it won't do anything unless awaited
        inner_lock.cleanup_handle = Some(tokio::spawn(async {
            sleep(Duration::from_secs(CLEANUP_WAIT_DURATION)).await;
            clean_up_fut.await;
        }));
    }
}
