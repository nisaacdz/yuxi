use chrono::{DateTime, Utc};
use std::collections::HashMap;
use tokio::{sync::Mutex, task::JoinHandle};

lazy_static::lazy_static! {
    pub static ref SCHEDULES: Mutex<HashMap<String, JoinHandle<()>>> = Mutex::new(HashMap::new());
}

pub async fn schedule_new_task<Fut>(
    id: String,
    task: Fut,
    scheduled_for: DateTime<Utc>,
) -> Result<(), String>
where
    Fut: Future<Output = ()> + Send + Sync + 'static,
{
    let mut schedules = SCHEDULES.lock().await;

    if schedules.contains_key(&id) {
        return Err("A task has already been scheduled with the same id".into());
    }

    let time_diff = scheduled_for - Utc::now();
    if let Ok(time_diff) = time_diff.to_std() {
        let task_id = id.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(time_diff).await;
            let mut schedules = SCHEDULES.lock().await;
            let handle = schedules.remove(&task_id);
            std::mem::drop(schedules);
            if let Some(_) = handle {
                task.await
            }
        });
        schedules.insert(id, handle);
        Ok(())
    } else {
        return Err("Scheduled time is in the past".into());
    }
}

pub async fn abort_scheduled_task(task_id: &String) -> Result<(), String> {
    let mut schedules = SCHEDULES.lock().await;
    match schedules.remove(task_id) {
        Some(handle) => Ok(handle.abort()),
        None => Err("Task not found".into()),
    }
}
