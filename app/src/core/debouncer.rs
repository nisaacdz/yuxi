use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Configuration for the Debouncer's behavior.
#[derive(Debug, Clone, Copy)]
pub struct DebouncerConfig {
    /// The quiet period required after a trigger before the action is executed.
    pub debounce_duration: Duration,
    /// The maximum number of triggers to be debounced before forcing an immediate execution.
    pub max_stack_size: usize,
    /// The absolute maximum time allowed to pass between executions, regardless of trigger frequency.
    pub max_debounce_period: Duration,
}

/// The public handle for the debouncing mechanism.
///
/// It is cheap to clone and can be shared across multiple threads or tasks.
/// When the last clone of a Debouncer is dropped, its background task will
/// automatically shut down.
pub struct Debouncer {
    // We wrap the fields that cannot be simply cloned into an Arc, allowing the main
    // struct `Debouncer` to derive `Clone` easily.
    inner: Arc<DebouncerInner>,
}

struct DebouncerInner {
    tx: mpsc::UnboundedSender<()>,
    // The handle must be optional so we can `take()` it on shutdown.
    shutdown_handle: std::sync::Mutex<Option<JoinHandle<()>>>,
}

impl Clone for Debouncer {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

// Custom debug implementation to avoid showing the internal fields.
impl fmt::Debug for Debouncer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Debouncer").finish_non_exhaustive()
    }
}

impl Debouncer {
    /// Creates a new Debouncer.
    ///
    /// It spawns a background Tokio task to manage the debouncing logic.
    ///
    /// # Arguments
    ///
    /// * `action`: The function or closure to be executed when the debouncing logic fires.
    ///            It must be `Send + Sync + 'static`.
    /// * `config`: The configuration that specifies the debouncer's behavior.
    pub fn new<F>(action: F, config: DebouncerConfig) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        let (tx, rx) = mpsc::unbounded_channel();
        let action = Arc::new(action);

        let shutdown_handle = tokio::spawn(worker_loop(action, config, rx));

        let inner = Arc::new(DebouncerInner {
            tx,
            shutdown_handle: std::sync::Mutex::new(Some(shutdown_handle)),
        });

        Self { inner }
    }

    /// Triggers the debouncer's action.
    ///
    /// This method is non-blocking and can be called quickly and repeatedly.
    /// The debouncer's internal logic will determine when to actually execute the action.
    /// It is safe to call from any thread.
    pub fn trigger(&self) {
        // Send a message to the worker task. We ignore the result; if the send fails,
        // it means the receiver (the worker task) has already been dropped, which
        // implies the debouncer is shut down. There's nothing to do in that case.
        let _ = self.inner.tx.send(());
    }

    /// Shuts down the debouncer gracefully.
    ///
    /// This method ensures that if there is a pending action to be executed, it will run
    /// before the debouncer fully terminates. It consumes the Debouncer handle.
    pub async fn shutdown(&self) {
        let handle = self.inner.shutdown_handle.lock().unwrap().take();

        if let Some(handle) = handle {
            // Drop our sender instance by closing the channel.
            self.inner.tx.clone().closed().await;

            // Wait for the worker task to complete its final execution and exit.
            let _ = handle.await;
        }
    }
}

/// The core worker loop of the debouncer, running as a dedicated Tokio task.
// CORRECTED worker_loop (the only part that needs to change)
async fn worker_loop<F>(
    action: Arc<F>,
    config: DebouncerConfig,
    mut rx: mpsc::UnboundedReceiver<()>,
) where
    F: Fn() + Send + Sync + 'static,
{
    let mut stack_size: usize = 0;
    let mut last_execution_time = Instant::now();
    let mut debounce_deadline: Option<Instant> = None;

    // The fix is here: we add `receiver` to the argument list to avoid capturing `rx`.
    let execute_and_reset =
        |current_stack: &mut usize,
         last_exec_time: &mut Instant,
         deadline_opt: &mut Option<Instant>,
         receiver: &mut mpsc::UnboundedReceiver<()>| {
            // Only execute if there was a pending trigger.
            if *current_stack > 0 {
                (action)();
            }

            // Use the passed-in `receiver` argument.
            while receiver.try_recv().is_ok() {}

            // Reset all state for the next cycle.
            *current_stack = 0;
            *last_exec_time = Instant::now();
            *deadline_opt = None;
        };

    loop {
        let sleep_duration = match debounce_deadline {
            Some(deadline) => {
                let max_period_deadline = last_execution_time + config.max_debounce_period;
                std::cmp::min(deadline, max_period_deadline)
                    .saturating_duration_since(Instant::now())
            }
            None => config.max_debounce_period,
        };

        tokio::select! {
            biased;

            msg = rx.recv() => {
                match msg {
                    Some(()) => {
                        stack_size += 1;
                        if stack_size >= config.max_stack_size {
                            // Update the call here...
                            execute_and_reset(&mut stack_size, &mut last_execution_time, &mut debounce_deadline, &mut rx);
                        } else {
                            debounce_deadline = Some(Instant::now() + config.debounce_duration);
                        }
                    },
                    None => {
                        break;
                    }
                }
            }

            _ = tokio::time::sleep(sleep_duration), if stack_size > 0 => {
                 // ...and update the call here.
                execute_and_reset(&mut stack_size, &mut last_execution_time, &mut debounce_deadline, &mut rx);
            }
        };
    }

    // ...and in the final cleanup call.
    execute_and_reset(
        &mut stack_size,
        &mut last_execution_time,
        &mut debounce_deadline,
        &mut rx,
    );
}


#[tokio::test]
async fn test_debouncer() {
    use std::time::SystemTime;
    println!("--- Demo Started ---");
    println!("Demonstrating a debouncer that prints a message.");
    println!("Config: 2s debounce, max 5 stack, max 4s total.");
    let start_time = SystemTime::now();

    let debouncer = Debouncer::new(
        move || {
            println!(
                "======> ACTION EXECUTED at ~{:.2}s <======",
                start_time.elapsed().unwrap_or_default().as_secs_f32()
            );
        },
        DebouncerConfig {
            debounce_duration: Duration::from_secs(2),
            max_stack_size: 5,
            max_debounce_period: Duration::from_secs(4),
        },
    );

    // --- Scenario 1: Basic Debouncing ---
    println!("\n[Scenario 1] Rapid calls being debounced.");
    for i in 1..=4 {
        println!("Triggering ({}/4)...", i);
        debouncer.trigger();
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    println!("Waiting for debounce duration (2s) to pass...");
    tokio::time::sleep(Duration::from_secs(3)).await;

    // --- Scenario 2: Max Stack Size Limit ---
    println!("\n[Scenario 2] Triggering more than max_stack_size (5) times.");
    for i in 1..=6 {
        println!("Triggering (stack should be {})...", i);
        debouncer.trigger();
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    // Execution will have happened on the 5th trigger. After another 2s, the 6th trigger will fire.
    println!("Waiting for final trigger's debounce to fire...");
    tokio::time::sleep(Duration::from_secs(3)).await;

    // --- Scenario 3: Max Debounce Period ---
    println!("\n[Scenario 3] Continuous triggers keeping it debounced, until max period (4s).");
    debouncer.trigger();
    println!("First trigger at t=0.");
    tokio::time::sleep(Duration::from_millis(1500)).await;
    debouncer.trigger();
    println!("Second trigger at t=~1.5s.");
    tokio::time::sleep(Duration::from_millis(1500)).await;
    debouncer.trigger();
    println!("Third trigger at t=~3.0s. (Execution should be forced at t=~4s from first trigger).");
    tokio::time::sleep(Duration::from_secs(2)).await; // Wait for the max period to fire.

    println!("\n--- Demo Finished ---");
    println!("\nExplicitly shutting down.");
    debouncer.shutdown().await;
    println!("Shutdown complete!");
}
