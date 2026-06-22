use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AuthorizeRequest {
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: String,
    pub state: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub created_at: Instant,
}

#[derive(Default)]
pub struct AuthorizeStash {
    inner: Mutex<HashMap<String, AuthorizeRequest>>,
    ttl: Duration,
}

impl AuthorizeStash {
    pub fn new(ttl: Duration) -> Self {
        AuthorizeStash { inner: Mutex::new(HashMap::new()), ttl }
    }

    pub fn insert(&self, mut req: AuthorizeRequest) -> String {
        req.created_at = Instant::now();
        let key = Uuid::new_v4().to_string();
        let mut guard = self.inner.lock().expect("authorize stash poisoned");
        // Opportunistic GC.
        let now = Instant::now();
        guard.retain(|_, v| now.duration_since(v.created_at) < self.ttl);
        guard.insert(key.clone(), req);
        key
    }

    pub fn take(&self, key: &str) -> Option<AuthorizeRequest> {
        let mut guard = self.inner.lock().expect("authorize stash poisoned");
        guard.remove(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    fn req() -> AuthorizeRequest {
        AuthorizeRequest {
            client_id: "synapse".into(),
            redirect_uri: "https://chat.hearth.local/_synapse/client/oidc/callback".into(),
            scope: "openid profile email".into(),
            state: "abc".into(),
            code_challenge: "xyz".into(),
            code_challenge_method: "S256".into(),
            created_at: Instant::now(),
        }
    }

    #[test]
    fn insert_then_take_returns_request() {
        let s = AuthorizeStash::new(Duration::from_secs(60));
        let key = s.insert(req());
        let r = s.take(&key).unwrap();
        assert_eq!(r.client_id, "synapse");
        assert!(s.take(&key).is_none(), "second take should miss");
    }

    #[test]
    fn ttl_expires_old_entries() {
        let s = AuthorizeStash::new(Duration::from_millis(10));
        let key = s.insert(req());
        sleep(Duration::from_millis(50));
        // Trigger GC by inserting another.
        let _ = s.insert(req());
        assert!(s.take(&key).is_none(), "expired entry should be gone");
    }
}
