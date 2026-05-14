use crate::task::ScheduledTask;

pub(crate) fn generate_task_id() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();
    (0..8)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

pub(crate) fn find_unique_id(
    tasks: &[ScheduledTask],
    mut id_source: impl FnMut() -> String,
) -> String {
    let mut id = id_source();
    while tasks.iter().any(|t| t.id == id) {
        id = id_source();
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expression::CronExpression;
    use chrono::{DateTime, Utc};

    fn sample_task(id: &str) -> ScheduledTask {
        let now: DateTime<Utc> = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        ScheduledTask {
            id: id.into(),
            cron: CronExpression::parse_at("* * * * *", now).unwrap(),
            prompt: String::new(),
            recurring: true,
            created_at: now,
            last_fired: None,
            durable: false,
        }
    }

    #[test]
    fn find_unique_id_returns_first_when_no_collision() {
        let tasks = vec![sample_task("abc")];
        let mut calls = 0;
        let picked = find_unique_id(&tasks, || {
            calls += 1;
            "xyz".to_string()
        });
        assert_eq!(picked, "xyz");
        assert_eq!(calls, 1);
    }

    #[test]
    fn find_unique_id_retries_on_collision() {
        let tasks = vec![sample_task("dup1"), sample_task("dup2")];
        let sequence = vec!["dup1".to_string(), "dup2".to_string(), "fresh".to_string()];
        let mut iter = sequence.into_iter();
        let picked = find_unique_id(&tasks, || iter.next().unwrap());
        assert_eq!(picked, "fresh");
    }

    #[test]
    fn find_unique_id_on_empty_tasks_accepts_anything() {
        let tasks: Vec<ScheduledTask> = Vec::new();
        let picked = find_unique_id(&tasks, || "only".into());
        assert_eq!(picked, "only");
    }
}
