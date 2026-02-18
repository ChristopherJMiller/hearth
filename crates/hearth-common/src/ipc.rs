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
