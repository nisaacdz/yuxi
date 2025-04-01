use std::{
    future::Future,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{sync::Mutex, task::JoinHandle, time::sleep};

// Renamed for clarity
struct TimeoutInner<C> {
    last_fut_completion: Option<Instant>, // Renamed
    cleanup_fn: Arc<C>,                   // Store cleanup behind Arc
    cleanup_handle: Option<JoinHandle<()>>,
}

/// Executes a cleanup function if a specified duration elapses after the
/// completion of the last submitted task. Submitting a new task resets the timer.
pub struct TimeoutMonitor<C> {
    inner: Arc<Mutex<TimeoutInner<C>>>,
    /// The duration of inactivity after the last task completion before
    /// the cleanup function is executed.
    cleanup_wait_duration: Duration, // Make it a field
}

impl<Fut, C> TimeoutMonitor<C>
where
    // Constraint for the cleanup function
    Fut: Future<Output = ()> + Send + Sync + 'static,
    C: Fn() -> Fut + Send + Sync + 'static, // Assuming Fn, wrapped in Arc
{
    /// Creates a new TimeoutMonitor.
    ///
    /// # Arguments
    ///
    /// * `cleanup`: The function (`Fn`) to execute after inactivity. It should return a future.
    /// * `cleanup_wait_duration`: The duration of inactivity required to trigger the `cleanup` function.
    pub fn new(cleanup: C, cleanup_wait_duration: Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(TimeoutInner {
                last_fut_completion: None,
                cleanup_fn: Arc::new(cleanup), // Wrap in Arc
                cleanup_handle: None,
            })),
            cleanup_wait_duration, // Store duration
        }
    }

    /// Submits a future (`fut`) for execution.
    ///
    /// The monitor will execute `fut`. After `fut` completes, a timer starts.
    /// If `cleanup_wait_duration` elapses without another call to `call`,
    /// the `cleanup` function provided at construction will be executed.
    /// If `call` is invoked again before the timer expires, the timer is reset.
    ///
    /// # Arguments
    ///
    /// * `fut`: A future representing the task to execute. Must be `Send + Sync + 'static`.
    pub async fn call<TaskFut>(&self, fut: TaskFut)
    // Use different generic name for task's future
    where
        TaskFut: Future<Output = ()> + Send + Sync + 'static, // Constraint for the task future
    {
        let mut inner_lock = self.inner.lock().await;

        // 1. Abort any existing cleanup task and clear its handle
        if let Some(cleanup_handle) = inner_lock.cleanup_handle.take() {
            // println!("Call: Aborting previous cleanup task.");
            cleanup_handle.abort();
        }

        // Drop lock before spawning task to avoid holding it across await potentially
        drop(inner_lock);

        // Clone Arc for the spawned task
        let inner_arc = self.inner.clone();
        let cleanup_wait = self.cleanup_wait_duration; // Capture duration

        // 2. Spawn the main task execution wrapper
        tokio::spawn(async move {
            // a. Execute the provided future
            fut.await;

            let finish_time = Instant::now();
            // println!("Task completed at {:?}", finish_time);

            // b. Lock shared state *after* fut completes
            let mut inner_lock = inner_arc.lock().await;

            // c. Update completion time
            inner_lock.last_fut_completion = Some(finish_time);

            // d. Schedule the new cleanup task
            let cleanup_fn_arc = inner_lock.cleanup_fn.clone(); // Clone Arc<C>
            let inner_clone_for_cleanup = inner_arc.clone();

            // println!("Scheduling cleanup task with wait: {:?}", cleanup_wait);
            let new_cleanup_handle = tokio::spawn(async move {
                sleep(cleanup_wait).await;
                // println!("Cleanup task: Woke up after {:?}", cleanup_wait);

                // Lock after sleep
                let mut cleanup_inner_lock = inner_clone_for_cleanup.lock().await;

                // CRUCIAL CHECK: Verify this cleanup task is still the valid one.
                // If cleanup_handle is None, it means call() aborted us after we started sleeping.
                if cleanup_inner_lock.cleanup_handle.is_none() {
                    // println!("Cleanup task: Exiting - Aborted during sleep.");
                    return; // Our handle was taken and aborted, do nothing.
                }

                // We are the valid task. Take the handle to prevent double execution
                // and signal that cleanup is proceeding or finished.
                // Note: This relies on comparing JoinHandles implicitly. For robustness,
                // using task IDs or generation counts might be better, but adds complexity.
                // Let's assume JoinHandle cancellation works reliably here.
                // We take the handle, signalling cleanup is starting.
                let _ = cleanup_inner_lock.cleanup_handle.take(); // Consume the handle

                // Clone Arc<C> again (or use the one captured earlier)
                let cleanup_to_run = cleanup_fn_arc; // Use captured Arc<C>

                //println!("Cleanup task: Preparing to run cleanup function.");

                // Drop lock *before* executing potentially long cleanup
                drop(cleanup_inner_lock);

                // Execute cleanup function
                cleanup_to_run().await;
                // println!("Cleanup task: Cleanup function finished.");

                // Re-acquire lock briefly to update state if needed
                let mut final_lock = inner_clone_for_cleanup.lock().await;
                // Reset completion time? User code did this.
                final_lock.last_fut_completion = None;
                // Ensure handle is None (already taken, but for clarity)
                // final_lock.cleanup_handle = None; // Already None because we took it
                // println!("Cleanup task: Reset last completion time.");
            });

            // Store the handle to the *new* cleanup task
            inner_lock.cleanup_handle = Some(new_cleanup_handle);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::time::sleep;

    async fn sample_task(id: u32, delay_ms: u64) {
        println!("Task {}: Starting (will take {}ms).", id, delay_ms);
        sleep(Duration::from_millis(delay_ms)).await;
        println!("Task {}: Finished.", id);
    }

    // Use AtomicBool to track if cleanup ran
    static CLEANUP_RAN: AtomicBool = AtomicBool::new(false);

    async fn sample_cleanup() {
        println!("Cleanup: Starting (will take 50ms).");
        sleep(Duration::from_millis(50)).await;
        CLEANUP_RAN.store(true, Ordering::SeqCst);
        println!("Cleanup: Finished.");
    }

    #[tokio::test]
    async fn test_cleanup_runs_after_timeout() {
        println!("\n--- test_cleanup_runs_after_timeout ---");
        CLEANUP_RAN.store(false, Ordering::SeqCst);
        let wait = Duration::from_millis(200);
        let monitor = TimeoutMonitor::new(sample_cleanup, wait);

        monitor.call(sample_task(1, 100)).await; // Task takes 100ms

        // Wait less than task time + cleanup wait (100 + 200 = 300ms)
        println!("Waiting 250ms (Task finishes + partial wait)...");
        sleep(Duration::from_millis(250)).await;
        assert!(
            !CLEANUP_RAN.load(Ordering::SeqCst),
            "Cleanup should not have run yet"
        );

        // Wait longer than task time + cleanup wait
        println!("Waiting another 100ms (Total > 300ms)...");
        sleep(Duration::from_millis(100)).await; // Total wait = 350ms
        assert!(
            CLEANUP_RAN.load(Ordering::SeqCst),
            "Cleanup should have run"
        );
        println!("--- END test_cleanup_runs_after_timeout ---");
    }

    #[tokio::test]
    async fn test_call_resets_timeout() {
        println!("\n--- test_call_resets_timeout ---");
        CLEANUP_RAN.store(false, Ordering::SeqCst);
        let wait = Duration::from_millis(200);
        let monitor = TimeoutMonitor::new(sample_cleanup, wait);

        monitor.call(sample_task(1, 50)).await; // Task 1 takes 50ms

        // Wait more than task 1 finishes, but less than cleanup wait (50 < wait < 200)
        println!("Waiting 150ms (after Task 1 finishes, before cleanup)...");
        sleep(Duration::from_millis(150)).await; // Total elapsed: 150ms
        assert!(
            !CLEANUP_RAN.load(Ordering::SeqCst),
            "Cleanup should not run before reset"
        );

        println!("Calling again to reset timer...");
        monitor.call(sample_task(2, 50)).await; // Task 2 takes 50ms. Resets timer based on *its* completion.

        // Wait should now be relative to Task 2's completion.
        // Task 2 finishes around 150ms + 50ms = 200ms total elapsed time.
        // Cleanup timer starts from ~200ms. Cleanup should run at ~200ms + 200ms = ~400ms.

        // Wait until just before cleanup should run (e.g., 380ms total elapsed)
        // Current elapsed is 150ms + 50ms = 200ms. Wait another 180ms.
        println!("Waiting another 180ms (Total elapsed ~380ms, just before reset cleanup)...");
        sleep(Duration::from_millis(180)).await;
        assert!(
            !CLEANUP_RAN.load(Ordering::SeqCst),
            "Cleanup should not run before reset timer expires"
        );

        // Wait past the reset cleanup time (e.g., 400ms total elapsed)
        // Need to wait another ~20ms+. Wait 50ms more.
        println!("Waiting another 50ms (Total elapsed ~430ms, past reset cleanup)...");
        sleep(Duration::from_millis(50)).await;
        assert!(
            CLEANUP_RAN.load(Ordering::SeqCst),
            "Cleanup should have run after reset timer expired"
        );
        println!("--- END test_call_resets_timeout ---");
    }

    #[tokio::test]
    async fn test_cleanup_does_not_run_if_called_continuously() {
        println!("\n--- test_cleanup_does_not_run_if_called_continuously ---");
        CLEANUP_RAN.store(false, Ordering::SeqCst);
        let wait = Duration::from_millis(200); // Cleanup wait
        let monitor = TimeoutMonitor::new(sample_cleanup, wait);

        let call_interval = Duration::from_millis(100); // Call more frequently than cleanup wait
        let task_duration = 50;

        for i in 0..5 {
            println!("Calling task {}...", i);
            // Don't await the monitor.call itself, just the task spawn within it
            // We need to ensure calls happen frequently relative to wait time
            monitor.call(sample_task(i, task_duration)).await;
            // Wait less than cleanup_wait_duration before the next call
            println!("Waiting interval {:?}...", call_interval);
            sleep(call_interval).await; // 100ms < 200ms wait
            assert!(
                !CLEANUP_RAN.load(Ordering::SeqCst),
                "Cleanup should not run during continuous calls"
            );
        }

        // Now, stop calling and wait for the timeout after the *last* task completion
        println!("Stopping calls. Waiting for final cleanup timeout...");
        // Last task finishes after ~5*100ms + 50ms = 550ms.
        // Cleanup should run 200ms after that, at ~750ms.
        sleep(wait + Duration::from_millis(100)).await; // Wait > 200ms

        assert!(
            CLEANUP_RAN.load(Ordering::SeqCst),
            "Cleanup should run after calls stop"
        );
        println!("--- END test_cleanup_does_not_run_if_called_continuously ---");
    }
}
