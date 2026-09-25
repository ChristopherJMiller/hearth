use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// TtlStash — generic short-lived, take-once value store keyed by a random id
// ---------------------------------------------------------------------------

/// A thread-safe stash of values keyed by a randomly generated id, each
/// living for at most `ttl`. Backs both the pending-`/authorize`-request
/// stash and the broker-issued-authorization-code stash: both need the same
/// "random key in, TTL-bounded take-once value out" bookkeeping and differ
/// only in the payload type.
///
/// GC runs opportunistically on `insert`, but `take` also re-checks the TTL
/// itself (returning `None` for an expired entry) so that a quiet server
/// can't let entries live forever between inserts.
pub struct TtlStash<T> {
    inner: Mutex<HashMap<String, (Instant, T)>>,
    ttl: Duration,
}

impl<T> TtlStash<T> {
    pub fn new(ttl: Duration) -> Self {
        TtlStash {
            inner: Mutex::new(HashMap::new()),
            ttl,
        }
    }

    /// Stash `value`, assign a random key, and return that key.
    pub fn insert(&self, value: T) -> String {
        let key = Uuid::new_v4().to_string();
        let now = Instant::now();
        let mut guard = self.inner.lock().expect("stash poisoned");
        // Opportunistic GC.
        guard.retain(|_, (created_at, _)| now.duration_since(*created_at) < self.ttl);
        guard.insert(key.clone(), (now, value));
        key
    }

    /// Remove and return the value for `key`, or `None` if missing or if it
    /// has outlived the TTL.
    pub fn take(&self, key: &str) -> Option<T> {
        let mut guard = self.inner.lock().expect("stash poisoned");
        let (created_at, value) = guard.remove(key)?;
        if Instant::now().duration_since(created_at) < self.ttl {
            Some(value)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod ttl_stash_tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn insert_then_take_returns_value() {
        let s = TtlStash::new(Duration::from_secs(60));
        let key = s.insert("hello");
        assert_eq!(s.take(&key), Some("hello"));
    }

    #[test]
    fn take_is_take_once() {
        let s = TtlStash::new(Duration::from_secs(60));
        let key = s.insert(42);
        assert_eq!(s.take(&key), Some(42));
        assert_eq!(s.take(&key), None, "second take should miss");
    }

    #[test]
    fn ttl_expires_old_entries_via_gc_at_insert() {
        let s = TtlStash::new(Duration::from_millis(10));
        let key = s.insert("stale");
        sleep(Duration::from_millis(50));
        // Trigger GC by inserting another.
        let _ = s.insert("fresh");
        assert!(s.take(&key).is_none(), "expired entry should be gone");
    }

    #[test]
    fn take_returns_none_for_expired_entry_even_without_gc_trigger() {
        // On a quiet server nothing else calls insert() to trigger GC, so
        // take() itself must enforce the TTL rather than returning a
        // long-stale entry.
        let s = TtlStash::new(Duration::from_millis(10));
        let key = s.insert("stale");
        sleep(Duration::from_millis(50));
        assert!(
            s.take(&key).is_none(),
            "take() must reject an entry older than the TTL"
        );
    }
}

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
}

/// In-memory stash for pending `/authorize` requests, keyed by the
/// `upstream_state` value sent to Kanidm.
pub type AuthorizeStash = TtlStash<AuthorizeRequest>;

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
}

/// Thread-safe stash for broker-issued authorization codes.
pub type BrokerCodeStash = TtlStash<BrokerCode>;

#[cfg(test)]
mod tests {
    use super::*;

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
}

#[cfg(test)]
mod broker_code_tests {
    use super::*;

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
}
