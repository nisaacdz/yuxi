use chrono::{DateTime, Utc};
use tokio::time::Instant;

pub fn schedule_new_task<Fut>(task: Fut, scheduled_for: DateTime<Utc>) -> Result<(), String>
where
    Fut: Future<Output = ()> + Send + 'static,
{
    let time_diff = scheduled_for - Utc::now();
    if let Ok(time_diff) = time_diff.to_std() {
        let deadline = Instant::now() + time_diff;
        tokio::spawn(async move {
            tokio::time::sleep_until(deadline).await;
            task.await;
        });
        Ok(())
    } else {
        Err("Scheduled time is in the past".into())
    }
}
