//! Offline event queue backed by SQLite.
//!
//! When the control plane is unreachable, events are queued locally.
//! The poller drains the queue when connectivity returns.

use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Mutex;
use tracing::{debug, info};

#[derive(Debug, Clone)]
pub struct QueuedEvent {
    #[allow(dead_code)]
    pub id: i64,
    pub event_type: String,
    pub payload: String,
}

pub struct OfflineQueue {
    conn: Mutex<Connection>,
}

impl OfflineQueue {
    pub fn open(path: &Path) -> Result<Self, rusqlite::Error> {
        if let Some(parent) = path.parent()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent).ok();
        }

        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS event_queue (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                event_type TEXT NOT NULL,
                payload    TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )?;

        info!(path = %path.display(), "offline queue opened");
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn enqueue(&self, event_type: &str, payload: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO event_queue (event_type, payload) VALUES (?1, ?2)",
            params![event_type, payload],
        )?;
        debug!(%event_type, "event queued for offline delivery");
        Ok(())
    }

    pub fn drain(&self) -> Result<Vec<QueuedEvent>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT id, event_type, payload FROM event_queue ORDER BY id ASC")?;
        let events: Vec<QueuedEvent> = stmt
            .query_map([], |row| {
                Ok(QueuedEvent {
                    id: row.get(0)?,
                    event_type: row.get(1)?,
                    payload: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !events.is_empty() {
            conn.execute("DELETE FROM event_queue", [])?;
            info!(count = events.len(), "drained offline queue");
        }
        Ok(events)
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM event_queue", [], |row| row.get(0))?;
        Ok(count == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_queue() -> (OfflineQueue, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_queue.db");
        let queue = OfflineQueue::open(&path).unwrap();
        (queue, dir)
    }

    #[test]
    fn enqueue_and_drain() {
        let (queue, _dir) = temp_queue();
        queue
            .enqueue("heartbeat", r#"{"machine_id":"abc"}"#)
            .unwrap();
        queue
            .enqueue("user_login", r#"{"username":"jdoe"}"#)
            .unwrap();
        assert!(!queue.is_empty().unwrap());
        let events = queue.drain().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "heartbeat");
        assert_eq!(events[1].event_type, "user_login");
        assert!(queue.is_empty().unwrap());
    }

    #[test]
    fn drain_empty_queue() {
        let (queue, _dir) = temp_queue();
        let events = queue.drain().unwrap();
        assert!(events.is_empty());
        assert!(queue.is_empty().unwrap());
    }
}
