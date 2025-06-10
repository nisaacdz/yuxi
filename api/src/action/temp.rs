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

// --- Inner Struct for Moderator ---
struct Inner<F, Fut>
where
    F: FnOnce() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    /// Number of calls accumulated/pending since the last processor execution was scheduled.
    pending_calls_count: usize,
    /// The processor function provided by the most recent `call`.
    next_processor: Option<F>,
    /// The current operational state of the monitor.
    state: State,
    /// Timestamp of when the last processor execution successfully completed.
    last_execution_finish_time: Option<Instant>,
    _phantom_fut: std::marker::PhantomData<Fut>,
}

// --- Generic Frequency Monitor Struct ---
/// Manages debounced and throttled execution of a `FnOnce()` processor based on invocation frequency,
/// number of pending calls, and time since the last execution.
pub struct Moderator<F, Fut>
where
    F: FnOnce() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    inner: Arc<Mutex<Inner<F, Fut>>>,
    debounce_duration: Duration,
    max_process_wait: Duration,
    /// If the number of pending calls reaches or exceeds this, execution is scheduled immediately (skipping debounce).
    max_pending_calls: usize,
}

impl<F, Fut> Moderator<F, Fut>
where
    F: FnOnce() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    pub fn new(
        debounce_duration: Duration,
        max_process_wait: Duration,
        max_pending_calls: usize,
    ) -> Self {
        if max_pending_calls == 0 {
            // Consider if 0 should mean "disabled" (usize::MAX) or "trigger always" (1).
            // For now, mirroring original behavior: warn if 0, as it might lead to excessive triggers.
            // A value of 1 means every call (if idle/debouncing) will attempt to schedule.
            eprintln!(
                "Warning: Moderator max_pending_calls is 0. This might lead to unexpected behavior or excessive triggers. Consider using 1 for immediate trigger on first call if debouncing is not desired initially."
            );
        }

        Self {
            inner: Arc::new(Mutex::new(Inner {
                pending_calls_count: 0,
                next_processor: None,
                state: State::Idle,
                last_execution_finish_time: None,
                _phantom_fut: std::marker::PhantomData,
            })),
            debounce_duration,
            max_process_wait,
            max_pending_calls,
        }
    }

    fn should_execute_immediately(&self, inner_data: &Inner<F, Fut>) -> bool {
        if inner_data.pending_calls_count >= self.max_pending_calls {
            return true;
        }
        if let Some(last_finish) = inner_data.last_execution_finish_time {
            if last_finish.elapsed() >= self.max_process_wait {
                return true;
            }
        }
        false
    }

    /// Signals an event and provides the `FnOnce` processor for the next execution cycle.
    ///
    /// This method handles the core logic:
    /// 1. Increments the `pending_calls_count`.
    /// 2. Stores the provided `processor`.
    /// 3. If `Idle` or `Debouncing`, checks `max_pending_calls` and `max_process_wait`.
    ///    - If conditions met, schedules immediate execution.
    ///    - Else, schedules with `debounce_duration`.
    /// 4. Aborts existing `Debouncing` task if any.
    /// 5. If `Processing`, the call is queued; completion logic handles the next cycle.
    pub async fn call(&self, processor: F) {
        let mut inner_lock = self.inner.lock().await;

        inner_lock.pending_calls_count += 1;
        inner_lock.next_processor = Some(processor);

        match inner_lock.state {
            State::Processing => {
                // Queued. Task completion logic will handle it.
            }
            State::Idle | State::Debouncing(_) => {
                if let State::Debouncing(handle) = &inner_lock.state {
                    handle.abort();
                }

                let should_sleep = !self.should_execute_immediately(&inner_lock);

                if let Some(proc_to_schedule) = inner_lock.next_processor.take() {
                    let handle = spawn_generic_processing_task(
                        self.inner.clone(),
                        proc_to_schedule,
                        should_sleep,
                        self.debounce_duration,
                        self.max_process_wait,
                        self.max_pending_calls,
                    );
                    inner_lock.state = State::Debouncing(handle);
                } else {
                    // This case should ideally not be reached if `call` always sets next_processor.
                    eprintln!(
                        "Moderator: CRITICAL Error - Processor vanished during Idle/Debouncing state handling."
                    );
                }
            }
        }
    }
}

fn spawn_generic_processing_task<F, Fut>(
    inner_mutex: Arc<Mutex<Inner<F, Fut>>>,
    processor_for_this_task: F, // The FnOnce() -> Fut
    should_sleep: bool,
    debounce_duration: Duration,
    max_process_wait: Duration, // For next cycle's decision by `call`
    max_pending_calls: usize,   // For next cycle's decision by `call`
) -> JoinHandle<()>
where
    F: FnOnce() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(async move {
        if should_sleep {
            sleep(debounce_duration).await;
        }

        let mut inner_lock = inner_mutex.lock().await;

        if !matches!(inner_lock.state, State::Debouncing(_)) {
            // Task was aborted or superseded.
            return;
        }

        // This task is committing to process.
        // The `processor_for_this_task` is the one from the latest `call` that triggered this scheduling.
        // All `pending_calls_count` up to this point are considered "covered" by this execution.
        // Reset count as these calls are now being processed by this task instance.
        inner_lock.pending_calls_count = 0;
        inner_lock.state = State::Processing;

        drop(inner_lock); // Release lock before running processor

        processor_for_this_task().await; // Execute the generic processor

        let finish_time = Instant::now();
        let mut inner_lock = inner_mutex.lock().await; // Re-acquire lock

        inner_lock.last_execution_finish_time = Some(finish_time);

        // Check for new work (a new processor) that arrived *during* this processing.
        let maybe_next_processor = inner_lock.next_processor.take();

        if let Some(next_proc_from_queue) = maybe_next_processor {
            // New calls arrived, inner_lock.pending_calls_count would have been incremented by them.
            // Decide if the *next* task should sleep based on the new count.
            let next_should_sleep = inner_lock.pending_calls_count < max_pending_calls;

            let handle = spawn_generic_processing_task(
                inner_mutex.clone(),
                next_proc_from_queue,
                next_should_sleep,
                debounce_duration,
                max_process_wait,
                max_pending_calls,
            );
            inner_lock.state = State::Debouncing(handle);
        } else {
            // No new calls (and thus no new processor) came in while processing.
            inner_lock.state = State::Idle;
            // Ensure count is zero if we are going idle and no processor is pending.
            inner_lock.pending_calls_count = 0;
        }
    })
}

// // These types are for the Moderator instance within FrequencyMonitor.
// // FMod is the type of the FnOnce() closure passed to Moderator.
// // FutMod is the type of the Future returned by FMod.
// type FMod = Box<dyn FnOnce() -> FutMod + Send + Sync + 'static>;
// type FutMod = Box<dyn Future<Output = ()> + Send + 'static>;

// /// A frequency monitor that accumulates characters and processes them in batches.
// /// Built on top of the generic `Moderator`.
// pub struct FrequencyMonitor<FUser, FutUser>
// where
//     FUser: FnOnce(Vec<char>) -> FutUser + Send + Sync + 'static,
//     FutUser: Future<Output = ()> + Send + 'static,
// {
//     moderator: Moderator<FMod, FutMod>,
//     accumulated_chars: Arc<Mutex<Vec<char>>>,
//     _phantom_user: std::marker::PhantomData<(FUser, FutUser)>,
// }

// impl<FUser, FutUser> FrequencyMonitor<FUser, FutUser>
// where
//     FUser: FnOnce(Vec<char>) -> FutUser + Send + Sync + 'static,
//     FutUser: Future<Output = ()> + Send + 'static,
// {
//     /// Creates a new `FrequencyMonitor`.
//     ///
//     /// # Arguments
//     ///
//     /// * `debounce_duration`: Standard delay after the last input before processing.
//     /// * `max_process_wait`: Max idle time after last execution, after which debouncing is skipped.
//     /// * `max_process_stack_size`: Max accumulated characters before debouncing is skipped.
//     ///   This maps to `max_pending_calls` for the underlying `Moderator`.
//     pub fn new(
//         debounce_duration: Duration,
//         max_process_wait: Duration,
//         max_process_stack_size: usize,
//     ) -> Self {
//         if max_process_stack_size == 0 {
//             eprintln!("Warning: FrequencyMonitor max_process_stack_size is 0. This might lead to unexpected behavior or excessive triggers. Consider using 1 for immediate trigger on first char if debouncing is not desired initially.");
//         }
//         Self {
//             moderator: Moderator::new(
//                 debounce_duration,
//                 max_process_wait,
//                 max_process_stack_size, // max_process_stack_size becomes max_pending_calls
//             ),
//             accumulated_chars: Arc::new(Mutex::new(Vec::new())),
//             _phantom_user: std::marker::PhantomData,
//         }
//     }

//     /// Adds a character to the accumulator and signals the underlying `Moderator`.
//     ///
//     /// The `user_processor` provided here will be executed by the `Moderator`
//     /// when conditions are met, with all characters accumulated up to that point.
//     /// If this method is called multiple times before the `Moderator` processes,
//     /// the `user_processor` from the latest call will be used.
//     pub async fn call(&self, c: char, user_processor: FUser) {
//         // 1. Accumulate character
//         self.accumulated_chars.lock().await.push(c);

//         // 2. Create the processor for the Moderator.
//         // This closure captures `self.accumulated_chars` (cloned Arc)
//         // and the `user_processor` (moved).
//         let chars_arc_clone = self.accumulated_chars.clone();

//         let moderator_level_processor: FMod = Box::new(move || {
//             // This async block is the actual future (FutMod) the Moderator will await.
//             let fut = async move {
//                 let mut chars_guard = chars_arc_clone.lock().await;
//                 let chars_to_process = std::mem::take(&mut *chars_guard);
//                 drop(chars_guard); // Release lock before calling user_processor

//                 // Only call the user_processor if there are characters.
//                 // This matches the behavior of the original FrequencyMonitor's task.
//                 if !chars_to_process.is_empty() {
//                     user_processor(chars_to_process).await;
//                 }
//             };
//             Box::pin(fut) as FutMod
//         });

//         // 3. Call the Moderator.
//         // Each call to `FrequencyMonitor::call` (i.e., for each char) results in
//         // one call to `moderator.call`. The `moderator`'s `pending_calls_count`
//         // will thus correspond to the number of accumulated characters.
//         self.moderator.call(moderator_level_processor).await;
//     }
// }
