use std::{future::Future, sync::Arc, time::Duration};
use tokio::{sync::Mutex, task::JoinHandle, time::sleep};

#[derive(Debug)]
enum TimeoutState<Fut> {
    Active {
        cleanup_fut: Fut,
        current_cleanup_handle: Option<JoinHandle<()>>,
    },
    TimedOut,
}

pub struct TimeoutMonitor<FAfter, Fut, AFutAfter>
where
    Fut: Future<Output = ()> + Send + 'static,
    FAfter: Fn() -> AFutAfter + Send + Sync + 'static,
    AFutAfter: Future<Output = ()> + Send + 'static,
{
    inner: Arc<Mutex<TimeoutState<Fut>>>,
    cleanup_wait_duration: Duration,
    after_timeout_fn: FAfter,
}

impl<FAfter, Fut, AFutAfter> TimeoutMonitor<FAfter, Fut, AFutAfter>
where
    Fut: Future<Output = ()> + Send + 'static,
    FAfter: Fn() -> AFutAfter + Send + Sync + Clone + 'static,
    AFutAfter: Future<Output = ()> + Send + 'static,
{
    /// Creates a new TimeoutMonitor.
    ///
    /// # Arguments
    ///
    /// * `cleanup_fn`: An async FnOnce that returns the primary cleanup future (`Fut`).
    ///                           This factory is called *immediately*, but the returned future is
    ///                           only awaited after the first inactivity period.
    /// * `after_timeout_fn`: An async Fn returning a Future (`AFutAfter`) to execute if `call`
    ///                            is invoked *after* the initial cleanup has occurred.
    /// * `cleanup_wait_duration`: The duration of inactivity required to trigger a cleanup action.
    pub fn new<C>(
        cleanup_fn: C, // Takes the FnOnce factory
        after_timeout_fn: FAfter,
        cleanup_wait_duration: Duration,
    ) -> Self
    where
        C: FnOnce() -> Fut,
    {
        let cleanup_fut = cleanup_fn();

        Self {
            inner: Arc::new(Mutex::new(TimeoutState::Active {
                cleanup_fut,
                current_cleanup_handle: None,
            })),
            cleanup_wait_duration,
            after_timeout_fn,
        }
    }

    /// Submits a future (`task_fut`) for execution under monitoring.
    /// If the monitor is `Active`, it resets the inactivity timer. If the timer expires,
    /// the original cleanup future runs, and the state transitions to `TimedOut`.
    /// If the monitor is `TimedOut`, this executes the `after_cleanup` action
    /// and resets the state to `Active` (using `after_cleanup` for subsequent timeouts).
    /// --> Modification: Let's clarify reset behavior. Does `TimedOut` state persist, or does `after_cleanup` reset it?
    /// --> Assuming `after_cleanup` is a *reaction* to calling when TimedOut, and doesn't reset the main state machine's core purpose.
    /// --> Let's refine: If `TimedOut`, `call` runs `after_cleanup`, then proceeds to run `task_fut` normally, effectively resetting.
    pub async fn call<TaskFut>(&self, task: TaskFut)
    where
        TaskFut: Future<Output = ()> + Send + Sync + 'static,
    {
        let inner_arc = self.inner.clone();
        let cleanup_wait = self.cleanup_wait_duration;
        let after_timeout_fn = self.after_timeout_fn.clone(); // Clone upfront

        let mut inner_lock = inner_arc.lock().await;

        match &mut *inner_lock {
            TimeoutState::Active {
                current_cleanup_handle,
                ..
            } => {
                if let Some(handle) = current_cleanup_handle.take() {
                    handle.abort();
                }
                execute_and_schedule_cleanup(inner_arc.clone(), task, cleanup_wait);
            }
            TimeoutState::TimedOut => {
                tokio::task::spawn(after_timeout_fn());
                return;
            }
        }
    }
}

fn execute_and_schedule_cleanup<Fut, TaskFut>(
    timeout_state: Arc<Mutex<TimeoutState<Fut>>>,
    task: TaskFut,
    cleanup_wait: Duration,
) where
    Fut: Future<Output = ()> + Send + 'static,
    TaskFut: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(async move {
        task.await;

        let timeout_state_clone = timeout_state.clone();

        let mut timeout_state_lock = timeout_state.lock().await;

        if let TimeoutState::Active {
            current_cleanup_handle,
            ..
        } = &mut *timeout_state_lock
        {
            if let Some(cleanup_handle) = current_cleanup_handle.take() {
                cleanup_handle.abort();
            }
            *current_cleanup_handle = Some(tokio::spawn(async move {
                sleep(cleanup_wait).await;
                let mut timeout_state_lock = timeout_state_clone.lock().await;
                match std::mem::replace(&mut *timeout_state_lock, TimeoutState::TimedOut) {
                    TimeoutState::Active { cleanup_fut, .. } => {
                        std::mem::drop(timeout_state_lock);
                        cleanup_fut.await;
                    }
                    TimeoutState::TimedOut => {
                        // Erm
                    }
                }
            }));
        }

        std::mem::drop(timeout_state_lock);
    });
}
