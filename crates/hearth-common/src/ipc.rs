//! IPC protocol types for communication between hearth-greeter and hearth-agent
//! via Unix domain socket.

use serde::{Deserialize, Serialize};

/// Requests sent from the greeter to the agent over the Unix socket.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AgentRequest {
    /// Health check.
    Ping,
    /// Request the agent to prepare a user's environment after authentication.
    PrepareUserEnv {
        username: String,
        #[serde(default)]
        groups: Vec<String>,
    },
    /// Query the current status of a user environment preparation.
    GetPrepareStatus { username: String },
    /// Dev-only fast-path: skip the control plane and activate a closure
    /// the caller already has in the local Nix store. Used by
    /// `just push-user-env` to shortcut the full closure→worker→Attic→
    /// fan-out→poll roundtrip (~60-120s) to a sub-5s host→agent push.
    /// Agent rejects this request unless `HEARTH_ENABLE_DEV_PUSH=1` is
    /// set in its environment — production deployments leave it unset.
    /// See `docs/rfc-001-push-fast-path.md` for the design rationale.
    ApplyClosure {
        username: String,
        /// Absolute /nix/store path of a per-user closure with an
        /// `activate` script at `<closure>/activate`.
        closure: String,
    },
}

/// Events sent from the agent to the greeter over the Unix socket.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AgentEvent {
    /// Response to Ping.
    Pong,
    /// User environment preparation has started.
    Preparing { username: String, message: String },
    /// Progress update during preparation.
    Progress {
        username: String,
        percent: u8,
        message: String,
    },
    /// User environment is ready; greeter may start the session.
    Ready { username: String },
    /// An error occurred during preparation.
    Error { username: String, message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    // Wire-format gates — these strings are part of the protocol that
    // crosses the Unix socket. The `just push-user-env` recipe hand-rolls
    // the JSON via `printf` and Python; the agent's serde derive
    // de-serializes it. Both ends must agree on the field names + tag,
    // so pin them here.
    #[test]
    fn apply_closure_request_serializes_to_expected_json() {
        let req = AgentRequest::ApplyClosure {
            username: "alice@kanidm".into(),
            closure: "/nix/store/abc-hearth-user-env".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(
            json,
            r#"{"type":"ApplyClosure","username":"alice@kanidm","closure":"/nix/store/abc-hearth-user-env"}"#
        );
    }

    #[test]
    fn apply_closure_request_round_trips() {
        let req = AgentRequest::ApplyClosure {
            username: "bob".into(),
            closure: "/nix/store/xyz".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: AgentRequest = serde_json::from_str(&json).unwrap();
        match back {
            AgentRequest::ApplyClosure { username, closure } => {
                assert_eq!(username, "bob");
                assert_eq!(closure, "/nix/store/xyz");
            }
            other => panic!("expected ApplyClosure, got {other:?}"),
        }
    }

    #[test]
    fn legacy_prepare_user_env_request_still_parses() {
        // Regression gate: adding ApplyClosure must not break the
        // existing greeter's PrepareUserEnv message format.
        let json = r#"{"type":"PrepareUserEnv","username":"alice","groups":["dev"]}"#;
        let req: AgentRequest = serde_json::from_str(json).unwrap();
        match req {
            AgentRequest::PrepareUserEnv { username, groups } => {
                assert_eq!(username, "alice");
                assert_eq!(groups, vec!["dev".to_string()]);
            }
            other => panic!("expected PrepareUserEnv, got {other:?}"),
        }
    }
}
