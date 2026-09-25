use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// AuthorizeStash — pending /authorize requests keyed by upstream_state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AuthorizeRequest {
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: String,
    pub state: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    /// The downstream app's OIDC nonce, echoed into the id_token we mint at
    /// /oidc/token. Synapse (and OIDC clients generally) reject id_tokens
    /// that don't echo the nonce they sent.
    pub nonce: Option<String>,
    /// The broker's own PKCE code verifier for the upstream (broker ->
    /// Kanidm) leg, independent of the downstream app's PKCE pair. Kanidm
    /// 1.10 enforces PKCE on OAuth2 clients by default.
    pub upstream_pkce_verifier: String,
    pub created_at: Instant,
}

#[derive(Default)]
pub struct AuthorizeStash {
    inner: Mutex<HashMap<String, AuthorizeRequest>>,
    ttl: Duration,
}

impl AuthorizeStash {
    pub fn new(ttl: Duration) -> Self {
        AuthorizeStash {
            inner: Mutex::new(HashMap::new()),
            ttl,
        }
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

    /// Remove and return the request for `key`, or `None` if missing or if
    /// it has outlived the TTL (GC only runs opportunistically on insert, so
    /// a quiet server could otherwise let entries live forever).
    pub fn take(&self, key: &str) -> Option<AuthorizeRequest> {
        let mut guard = self.inner.lock().expect("authorize stash poisoned");
        let entry = guard.remove(key)?;
        if Instant::now().duration_since(entry.created_at) < self.ttl {
            Some(entry)
        } else {
            None
        }
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
            nonce: Some("n-0S6_WzA2Mj".into()),
            upstream_pkce_verifier: "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into(),
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

    #[test]
    fn take_returns_none_for_expired_entry_even_without_gc_trigger() {
        // On a quiet server nothing else calls insert() to trigger GC, so
        // take() itself must enforce the TTL rather than returning a
        // long-stale entry.
        let s = AuthorizeStash::new(Duration::from_millis(10));
        let key = s.insert(req());
        sleep(Duration::from_millis(50));
        assert!(
            s.take(&key).is_none(),
            "take() must reject an entry older than the TTL"
        );
    }
}

// ---------------------------------------------------------------------------
// BrokerCodeStash — short-lived codes the broker issues to downstream apps
// ---------------------------------------------------------------------------

/// The broker's authorization code, stashed for 60 s until the downstream
/// app exchanges it at /oidc/token.
#[derive(Debug, Clone)]
pub struct BrokerCode {
    pub client_id: String,
    pub redirect_uri: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub user_sub: String,
    pub user_username: Option<String>,
    pub user_email: Option<String>,
    pub user_email_verified: Option<bool>,
    pub user_name: Option<String>,
    pub user_groups: Vec<String>,
    pub scope: String,
    pub nonce: Option<String>,
    pub created_at: Instant,
}

/// Thread-safe stash for broker-issued authorization codes.
#[derive(Default)]
pub struct BrokerCodeStash {
    inner: Mutex<HashMap<String, BrokerCode>>,
    ttl: Duration,
}

impl BrokerCodeStash {
    pub fn new(ttl: Duration) -> Self {
        BrokerCodeStash {
            inner: Mutex::new(HashMap::new()),
            ttl,
        }
    }

    /// Stash `code`, assign a random key, and return that key.
    pub fn insert(&self, mut code: BrokerCode) -> String {
        code.created_at = Instant::now();
        let key = Uuid::new_v4().to_string();
        let mut guard = self.inner.lock().expect("code stash poisoned");
        let now = Instant::now();
        guard.retain(|_, v| now.duration_since(v.created_at) < self.ttl);
        guard.insert(key.clone(), code);
        key
    }

    /// Remove and return the code for `key`, or `None` if missing or if it
    /// has outlived the TTL (GC only runs opportunistically on insert, so a
    /// quiet server could otherwise let entries live forever).
    pub fn take(&self, key: &str) -> Option<BrokerCode> {
        let mut guard = self.inner.lock().expect("code stash poisoned");
        let entry = guard.remove(key)?;
        if Instant::now().duration_since(entry.created_at) < self.ttl {
            Some(entry)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod broker_code_tests {
    use super::*;
    use std::thread::sleep;

    fn code() -> BrokerCode {
        BrokerCode {
            client_id: "synapse".into(),
            redirect_uri: "https://chat.hearth.local/_synapse/client/oidc/callback".into(),
            code_challenge: "xyz".into(),
            code_challenge_method: "S256".into(),
            user_sub: "u-1".into(),
            user_username: Some("alice".into()),
            user_email: Some("alice@hearth.local".into()),
            user_email_verified: Some(true),
            user_name: Some("Alice".into()),
            user_groups: vec!["hearth-users".into()],
            scope: "openid profile email".into(),
            nonce: Some("n-0S6_WzA2Mj".into()),
            created_at: Instant::now(),
        }
    }

    #[test]
    fn insert_then_take_returns_code() {
        let s = BrokerCodeStash::new(Duration::from_secs(60));
        let key = s.insert(code());
        let c = s.take(&key).unwrap();
        assert_eq!(c.user_sub, "u-1");
        assert_eq!(c.user_email_verified, Some(true));
        assert!(s.take(&key).is_none(), "second take should miss");
    }

    #[test]
    fn take_returns_none_for_expired_entry_even_without_gc_trigger() {
        let s = BrokerCodeStash::new(Duration::from_millis(10));
        let key = s.insert(code());
        sleep(Duration::from_millis(50));
        assert!(
            s.take(&key).is_none(),
            "take() must reject an entry older than the TTL"
        );
    }
}
