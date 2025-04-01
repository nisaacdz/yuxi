use std::{
    future::Future,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{sync::Mutex, task::JoinHandle, time::sleep};

#[derive(Debug)]
enum State {
    Idle,
    Debouncing(JoinHandle<()>),
    Processing,
}

struct Inner<F, Fut>
where
    F: FnOnce(Vec<char>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    /// Characters accumulated since the last processor execution started.
    accumulated_chars: Vec<char>,
    /// The processor function provided by the most recent `call`.
    /// This will be consumed by the next execution cycle. `None` if no call occurred since the last execution.
    next_processor: Option<F>,
    /// The current operational state of the monitor (`Idle`, `Debouncing`, `Processing`).
    state: State,
    /// Timestamp of when the last processor execution successfully completed.
    /// Used to calculate the `max_process_wait` condition. `None` initially or if never run.
    last_execution_finish_time: Option<Instant>,
    /// PhantomData to associate the `Fut` generic type parameter.
    _phantom_fut: std::marker::PhantomData<Fut>,
}

// --- Frequency Monitor Struct ---

/// Manages debounced and throttled execution of a `FnOnce` processor based on input frequency,
/// accumulated input size, and time since the last execution.
///
/// This struct allows accumulating character inputs (`call`) and executing a provided
/// processor function (`F`) after a specified `debounce_duration`. The debouncing can be
/// bypassed if either the `max_process_stack_size` or `max_process_wait` thresholds are met,
/// leading to immediate scheduling of the processor (though execution still waits if another
/// processor instance is currently running).
pub struct FrequencyMonitor<F, Fut>
where
    F: FnOnce(Vec<char>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    /**
     * Shared, mutable inner state protected by `Arc<Mutex>`.
     */
    inner: Arc<Mutex<Inner<F, Fut>>>,

    /**
     * The duration to wait after the last character input before scheduling the processor,
     * unless overridden by `max_process_wait` or `max_process_stack_size`.
     */
    debounce_duration: Duration,
    /**
     * If the duration since the *last processor execution finished* exceeds this value
     * when `call` is invoked, the processor execution scheduled by that `call`
     * will skip the `debounce_duration` sleep. This ensures timely processing
     * after long periods of inactivity.
     * Set to a very large value (e.g., `Duration::MAX`) to effectively disable this trigger.
     */
    max_process_wait: Duration,

    /**
     * If the number of accumulated characters reaches or exceeds this size after a `call`,
     * the processor execution scheduled by that `call` will skip the `debounce_duration` sleep.
     * This prevents unbounded accumulation during rapid input bursts.
     * Set to `usize::MAX` to effectively disable this trigger.
     */
    max_process_stack_size: usize,
}

// --- Implementation ---

impl<F, Fut> FrequencyMonitor<F, Fut>
where
    F: FnOnce(Vec<char>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    /// Creates a new `FrequencyMonitor` with specified configuration.
    ///
    /// # Arguments
    ///
    /// * `debounce_duration`: The standard delay after the last input before processing.
    /// * `max_process_wait`: The maximum idle time after the last execution, after which debouncing is skipped. Use `Duration::MAX` to disable.
    /// * `max_process_stack_size`: The maximum number of accumulated characters before debouncing is skipped. Use `usize::MAX` to disable.
    pub fn new(
        debounce_duration: Duration,
        max_process_wait: Duration,
        max_process_stack_size: usize,
    ) -> Self {
        if max_process_stack_size == 0 {
            eprintln!("Warning: max_process_stack_size is 0, processor may trigger excessively.");
        }

        Self {
            inner: Arc::new(Mutex::new(Inner {
                accumulated_chars: Vec::new(),
                next_processor: None,
                state: State::Idle,
                last_execution_finish_time: None,
                _phantom_fut: std::marker::PhantomData,
            })),
            debounce_duration,
            max_process_wait,
            max_process_stack_size,
        }
    }

    /// Helper to determine if conditions for immediate execution are met.
    /// Operates on the locked inner state.
    fn should_execute_immediately(&self, inner_data: &Inner<F, Fut>) -> bool {
        // Check stack size condition first (cheaper)
        if inner_data.accumulated_chars.len() >= self.max_process_stack_size {
            // println!("Trigger: Stack size limit reached.");
            return true;
        }

        // Check max wait condition using the time the *last* execution finished
        if let Some(last_finish) = inner_data.last_execution_finish_time {
            if last_finish.elapsed() >= self.max_process_wait {
                // println!("Trigger: Max process wait time exceeded.");
                return true;
            }
        }

        // No immediate trigger condition met
        false
    }

    /// Adds a character to the accumulator and provides the `FnOnce` processor for the next execution cycle.
    ///
    /// This method handles the core logic:
    /// 1. Accumulates the character `c`.
    /// 2. Stores the provided `processor` (replacing any previously stored one for the next cycle).
    /// 3. If the monitor is `Idle` or `Debouncing`, it checks `max_process_stack_size` and `max_process_wait`.
    ///    - If either condition is met, it schedules the processor task for immediate execution (no initial sleep).
    ///    - Otherwise, it schedules the processor task with an initial sleep of `debounce_duration`.
    /// 4. If a `Debouncing` task already exists, it's aborted before scheduling the new one.
    /// 5. If the monitor is currently `Processing`, it simply queues the character and processor; the running task's
    ///    completion logic will handle scheduling the next cycle.
    ///
    /// # Arguments
    ///
    /// * `c`: The character to accumulate.
    /// * `processor`: The `FnOnce` closure to be executed for the accumulated characters. It consumes the characters (`Vec<char>`).
    pub async fn call(&self, c: char, processor: F) {
        let mut inner_lock = self.inner.lock().await;

        inner_lock.accumulated_chars.push(c);
        inner_lock.next_processor = Some(processor);

        match inner_lock.state {
            State::Processing => {
                // Currently processing, state updated. Do nothing more here.
                // Task completion logic will check accumulated chars & next_processor.
                // println!("Call: In Processing state. Queued '{}'.", c);
            }
            State::Idle | State::Debouncing(_) => {
                // Abort existing debounce task if present
                if let State::Debouncing(handle) = &inner_lock.state {
                    // println!("Call: Aborting existing debounce task.");
                    handle.abort();
                } else {
                    // println!("Call: In Idle state.");
                }

                // Check if immediate execution conditions are met NOW
                let should_sleep = !self.should_execute_immediately(&inner_lock);

                // Take the processor to schedule
                if let Some(proc_to_schedule) = inner_lock.next_processor.take() {
                    // println!("Call: Scheduling task (should_sleep: {}).", should_sleep);
                    let handle = spawn_processing_task(
                        self.inner.clone(),
                        proc_to_schedule,       // Move F into the task
                        should_sleep,           // Pass sleep decision for this specific task
                        self.debounce_duration, // Pass config values needed for task's logic & *next* scheduling
                        self.max_process_wait,
                        self.max_process_stack_size,
                    );
                    // Always transition to Debouncing, as a task is now scheduled (even if sleep is zero).
                    // The task itself will transition state to Processing internally.
                    inner_lock.state = State::Debouncing(handle);
                } else {
                    eprintln!(
                        "CRITICAL Error: Processor vanished during Idle/Debouncing state handling."
                    );
                }
            }
        }
    }
}

/// Spawns the asynchronous task responsible for potentially sleeping (debouncing),
/// executing the processor, and scheduling the next cycle if necessary.
///
/// This function takes ownership of the `processor_for_this_task` (`F`).
fn spawn_processing_task<F, Fut>(
    inner_mutex: Arc<Mutex<Inner<F, Fut>>>,
    processor_for_this_task: F,
    should_sleep: bool,
    debounce_duration: Duration,
    max_process_wait: Duration,
    max_process_stack_size: usize,
) -> JoinHandle<()>
where
    F: FnOnce(Vec<char>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(async move {
        // --- Phase 1: Pre-Processing (Potential Sleep) ---
        if should_sleep {
            // println!("Task: Sleeping for {:?}...", debounce_duration);
            sleep(debounce_duration).await;
        } else {
            // println!("Task: Skipping sleep.");
        }

        // --- Phase 2: Acquire Lock & Validate State ---
        let mut inner_lock = inner_mutex.lock().await;

        // Check if this task is still the active one.
        // If state is not Debouncing, it means we were aborted/superseded while sleeping/waiting.
        if !matches!(inner_lock.state, State::Debouncing(_)) {
            // println!("Task: Exiting - State was not Debouncing (likely aborted).");
            // processor_for_this_task is dropped implicitly.
            return;
        }

        // Take accumulated characters.
        let chars_to_process = std::mem::take(&mut inner_lock.accumulated_chars);

        // If no characters accumulated (edge case), don't process.
        if chars_to_process.is_empty() {
            // println!("Task: Exiting - No characters to process.");
            inner_lock.state = State::Idle;
            // processor_for_this_task is dropped implicitly.
            return;
        }

        // We are go for processing. Set state.
        // println!("Task: Transitioning to Processing state.");
        inner_lock.state = State::Processing;

        // --- Phase 3: Processor Execution ---
        // Drop the lock *before* running the potentially long processor.
        drop(inner_lock);

        // Execute the processor (consumes processor_for_this_task).
        // println!("Task: Executing processor for '{}'...", chars_to_process.iter().collect::<String>());
        processor_for_this_task(chars_to_process).await; // F is consumed here

        let finish_time = Instant::now(); // Record finish time immediately after await completes.
                                          // println!("Task: Processor execution finished.");

        // --- Phase 4: Post-Processing & Scheduling Next Cycle ---
        // Re-acquire the lock to update state.
        let mut inner_lock = inner_mutex.lock().await;
        // println!("Task: Re-acquired lock for post-processing.");

        // Defensive check: Ensure state is still Processing (it should be).
        if !matches!(inner_lock.state, State::Processing) {
            eprintln!("Warning: State was not 'Processing' after processor finished execution. Proceeding based on current state.");
        }

        // Update last execution finish time.
        inner_lock.last_execution_finish_time = Some(finish_time);

        // Check if new work (chars AND a processor) arrived *during* our execution.
        let maybe_next_processor = inner_lock.next_processor.take(); // Check and take if present

        if let Some(next_processor) = maybe_next_processor {
            // Decide if the *immediately following* task should sleep.
            // This check *only* depends on stack size here. The max_wait condition
            // will be checked properly by the *next* `call` before that task starts.
            let next_should_sleep = inner_lock.accumulated_chars.len() < max_process_stack_size;

            let handle = spawn_processing_task(
                inner_mutex.clone(),
                next_processor,
                next_should_sleep,
                debounce_duration,
                max_process_wait,
                max_process_stack_size,
            );
            inner_lock.state = State::Debouncing(handle);
        } else {
            // No pending characters OR no next processor was provided. Go Idle.
            inner_lock.state = State::Idle;

            // Cleanup: If there are leftover chars but no processor, they are stranded. Clear them.
            if !inner_lock.accumulated_chars.is_empty() {
                // println!("Task: Clearing {} stranded characters (no processor provided).", inner_lock.accumulated_chars.len());
                inner_lock.accumulated_chars.clear();
            }
            // If maybe_next_processor was Some but chars were empty, it's dropped implicitly here.
        }
        // Mutex lock released automatically
    })
}

// --- Example Usage (Requires updating tests to use new constructor) ---
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::time::{sleep, Duration};

    // Counter for processor calls
    static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

    async fn processor(chars: Vec<char>) {
        CALL_COUNT.fetch_add(1, Ordering::SeqCst);
        print!("|");
        for char in chars {
            sleep(Duration::from_millis(20)).await;
            print!("{}", char);
        }
    }

    // --- Test Cases Need updating for new() signature ---

    #[tokio::test]
    async fn test_basic_debounce() {
        println!("\n--- test_basic_debounce ---");
        CALL_COUNT.store(0, Ordering::SeqCst);
        // Standard debounce: 100ms, effectively disabled max_wait/stack
        let monitor = FrequencyMonitor::new(
            Duration::from_millis(100),
            Duration::from_secs(3600), // Large max wait
            1000,                      // Large stack size
        );

        monitor.call('a', processor).await;
        monitor.call('b', processor).await;
        sleep(Duration::from_millis(50)).await; // Less than debounce
        monitor.call('c', processor).await; // Last processor for this burst
        sleep(Duration::from_millis(150)).await; // Wait > 100ms debounce

        monitor.call('d', processor).await; // Start new debounce cycle
        sleep(Duration::from_millis(150)).await;
        println!();

        assert_eq!(
            CALL_COUNT.load(Ordering::SeqCst),
            2,
            "Expected two distinct processor runs"
        );
        println!("--- END test_basic_debounce ---");
    }

    #[tokio::test]
    async fn test_max_stack_size_trigger() {
        println!("\n--- test_max_stack_size_trigger ---");
        CALL_COUNT.store(0, Ordering::SeqCst);
        // Trigger immediate execution when 3 chars are reached
        let monitor = FrequencyMonitor::new(
            Duration::from_millis(500), // Long debounce (should be bypassed)
            Duration::from_secs(3600),
            3, // Trigger at 3 chars
        );

        monitor.call('a', processor).await; // 1 char
        monitor.call('b', processor).await; // 2 chars
        monitor.call('c', processor).await; // 3 chars -> SHOULD schedule immediately (no sleep)

        // Since it should schedule immediately, processing should start very soon after call('c')
        // Wait just enough for processing (50ms) + small buffer
        sleep(Duration::from_millis(100)).await;

        assert_eq!(
            CALL_COUNT.load(Ordering::SeqCst),
            1,
            "Expected immediate run due to stack size"
        );

        monitor.call('d', processor).await; // Start normal debounce for next run
        sleep(Duration::from_millis(550)).await; // Wait > 500ms debounce

        assert_eq!(
            CALL_COUNT.load(Ordering::SeqCst),
            2,
            "Expected second run after its debounce"
        );
        println!("--- END test_max_stack_size_trigger ---");
    }

    #[tokio::test]
    async fn test_max_wait_trigger() {
        println!("\n--- test_max_wait_trigger ---");
        CALL_COUNT.store(0, Ordering::SeqCst);
        // Trigger immediate if > 200ms since last run
        let monitor = FrequencyMonitor::new(
            Duration::from_millis(500), // Long debounce (should be bypassed by wait)
            Duration::from_millis(200), // Max wait trigger
            1000,
        );

        // First run normally after debounce
        monitor.call('a', processor).await;
        sleep(Duration::from_millis(550)).await; // Wait > debounce & > max_wait
        assert_eq!(
            CALL_COUNT.load(Ordering::SeqCst),
            1,
            "First run should complete"
        );

        // Wait longer than max_process_wait (200ms) but less than debounce (500ms)
        sleep(Duration::from_millis(250)).await;
        monitor.call('b', processor).await; // Should schedule immediately

        // Wait just enough for processing (50ms) + buffer
        sleep(Duration::from_millis(100)).await;

        assert_eq!(
            CALL_COUNT.load(Ordering::SeqCst),
            2,
            "Expected second run immediately due to max_wait"
        );
        println!("--- END test_max_wait_trigger ---");
    }

    #[tokio::test]
    async fn test_processing_overlap_with_config() {
        println!("\n--- test_processing_overlap_with_config ---");
        CALL_COUNT.store(0, Ordering::SeqCst);
        let monitor = FrequencyMonitor::new(
            Duration::from_millis(100), // Standard debounce
            Duration::from_secs(3600),
            1000,
        );

        monitor.call('x', processor).await;
        monitor.call('y', processor).await; // Last proc for first run

        // Wait for debounce (100ms) to trigger processing of "xy"
        sleep(Duration::from_millis(110)).await;

        // Processing of "xy" should have started (takes 50ms).
        // Call 'z' during this 50ms window.
        monitor.call('z', processor).await; // Store 'z' and proc B

        // Wait long enough for:
        // 1. overlap-A to finish (~50ms from its start)
        // 2. The standard debounce timer for overlap-B to start and finish (100ms)
        // 3. overlap-B processing to finish (~50ms)
        // Needs time for A (~50ms) + B's debounce (100ms) + B's processing (~50ms) = ~200ms total from when A started
        sleep(Duration::from_millis(250)).await; // Wait a bit more than the sum

        println!("Finished test_processing_overlap.");
        assert_eq!(
            CALL_COUNT.load(Ordering::SeqCst),
            2,
            "Expected two runs, A then B"
        );
        println!("--- END test_processing_overlap_with_config ---");
    }
}
