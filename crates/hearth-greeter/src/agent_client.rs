//! Agent Unix socket client.
//!
//! Communicates with hearth-agent over a Unix domain socket using newline-delimited
//! JSON, reusing the [`AgentRequest`] and [`AgentEvent`] types from `hearth_common::ipc`.

use hearth_common::ipc::{AgentEvent, AgentRequest};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tracing::{debug, trace};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum AgentClientError {
    #[error("IO error communicating with agent: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("connection closed by agent")]
    ConnectionClosed,
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// Client for the hearth-agent IPC Unix socket.
///
/// The protocol is newline-delimited JSON: each message is a single JSON object
/// followed by `\n`.
pub struct AgentClient {
    reader: BufReader<tokio::io::ReadHalf<UnixStream>>,
    writer: tokio::io::WriteHalf<UnixStream>,
}

impl AgentClient {
    /// Connect to the agent socket at `socket_path`.
    pub async fn connect(socket_path: &str) -> Result<Self, AgentClientError> {
        debug!(path = %socket_path, "connecting to agent socket");
        let stream = UnixStream::connect(socket_path).await?;
        let (read_half, write_half) = tokio::io::split(stream);
        Ok(Self {
            reader: BufReader::new(read_half),
            writer: write_half,
        })
    }

    /// Send a request to the agent.
    pub async fn send(&mut self, req: &AgentRequest) -> Result<(), AgentClientError> {
        let mut payload = serde_json::to_string(req)?;
        payload.push('\n');
        trace!(msg = %payload.trim(), "sending to agent");
        self.writer.write_all(payload.as_bytes()).await?;
        self.writer.flush().await?;
        Ok(())
    }

    /// Receive the next event from the agent.
    ///
    /// Blocks until a full line is available. Returns
    /// [`AgentClientError::ConnectionClosed`] if the agent closes the connection.
    pub async fn recv(&mut self) -> Result<AgentEvent, AgentClientError> {
        let mut line = String::new();
        let n = self.reader.read_line(&mut line).await?;
        if n == 0 {
            return Err(AgentClientError::ConnectionClosed);
        }
        trace!(msg = %line.trim(), "received from agent");
        let event: AgentEvent = serde_json::from_str(line.trim())?;
        Ok(event)
    }
}
