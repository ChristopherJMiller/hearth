//! greetd IPC client.
//!
//! greetd uses length-prefixed JSON over a Unix domain socket at `$GREETD_SOCK`.
//! Each message is a 4-byte little-endian length prefix followed by the JSON payload.

use serde::{Deserialize, Serialize};
use std::env;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tracing::{debug, trace};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum GreetdError {
    #[error("GREETD_SOCK environment variable not set")]
    NoSocket,
    #[error("IO error communicating with greetd: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unexpected end of stream from greetd")]
    UnexpectedEof,
}

// ---------------------------------------------------------------------------
// Wire types — outgoing requests
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
enum Request {
    #[serde(rename = "create_session")]
    CreateSession { username: String },
    #[serde(rename = "post_auth_message_response")]
    PostAuthMessageResponse { response: Option<String> },
    #[serde(rename = "start_session")]
    StartSession { cmd: Vec<String> },
    #[serde(rename = "cancel_session")]
    CancelSession,
}

// ---------------------------------------------------------------------------
// Wire types — incoming responses
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
enum RawResponse {
    #[serde(rename = "success")]
    Success,
    #[serde(rename = "auth_message")]
    AuthMessage {
        auth_message_type: String,
        auth_message: String,
    },
    #[serde(rename = "error")]
    Error {
        #[serde(default)]
        error_type: String,
        description: String,
    },
}

// ---------------------------------------------------------------------------
// Public response type
// ---------------------------------------------------------------------------

/// A response from the greetd daemon.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Response {
    /// The request succeeded.
    Success,
    /// greetd is requesting authentication information from the user.
    AuthMessage {
        /// `"secret"` for passwords, `"visible"` for echo-back prompts,
        /// `"info"` and `"error"` for informational messages.
        auth_message_type: String,
        /// The prompt/message text.
        auth_message: String,
    },
    /// An error occurred.
    Error {
        /// Human-readable error description.
        description: String,
    },
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// Client for the greetd IPC protocol.
pub struct GreetdClient {
    stream: UnixStream,
}

impl GreetdClient {
    /// Connect to the greetd daemon using the `GREETD_SOCK` environment variable.
    pub async fn connect() -> Result<Self, GreetdError> {
        let sock_path = env::var("GREETD_SOCK").map_err(|_| GreetdError::NoSocket)?;
        debug!(path = %sock_path, "connecting to greetd socket");
        let stream = UnixStream::connect(&sock_path).await?;
        Ok(Self { stream })
    }

    /// Ask greetd to create a new authentication session for `username`.
    pub async fn create_session(&mut self, username: &str) -> Result<Response, GreetdError> {
        let req = Request::CreateSession {
            username: username.to_string(),
        };
        self.send_request(&req).await?;
        self.read_response().await
    }

    /// Respond to an `auth_message` prompt. Pass `None` for informational
    /// messages that do not require a response.
    pub async fn post_auth_response(
        &mut self,
        response: Option<&str>,
    ) -> Result<Response, GreetdError> {
        let req = Request::PostAuthMessageResponse {
            response: response.map(String::from),
        };
        self.send_request(&req).await?;
        self.read_response().await
    }

    /// Start the session with the given command.
    pub async fn start_session(&mut self, cmd: &[&str]) -> Result<Response, GreetdError> {
        let req = Request::StartSession {
            cmd: cmd.iter().map(|s| s.to_string()).collect(),
        };
        self.send_request(&req).await?;
        self.read_response().await
    }

    /// Cancel the current authentication session.
    #[allow(dead_code)]
    pub async fn cancel_session(&mut self) -> Result<Response, GreetdError> {
        let req = Request::CancelSession;
        self.send_request(&req).await?;
        self.read_response().await
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    async fn send_request(&mut self, req: &Request) -> Result<(), GreetdError> {
        let payload = serde_json::to_vec(req)?;
        let len = payload.len() as u32;
        trace!(len, "sending greetd request");
        self.stream.write_all(&len.to_le_bytes()).await?;
        self.stream.write_all(&payload).await?;
        self.stream.flush().await?;
        Ok(())
    }

    async fn read_response(&mut self) -> Result<Response, GreetdError> {
        let mut len_buf = [0u8; 4];
        self.stream
            .read_exact(&mut len_buf)
            .await
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::UnexpectedEof => GreetdError::UnexpectedEof,
                _ => GreetdError::Io(e),
            })?;
        let len = u32::from_le_bytes(len_buf) as usize;
        trace!(len, "reading greetd response");

        let mut buf = vec![0u8; len];
        self.stream
            .read_exact(&mut buf)
            .await
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::UnexpectedEof => GreetdError::UnexpectedEof,
                _ => GreetdError::Io(e),
            })?;

        let raw: RawResponse = serde_json::from_slice(&buf)?;
        match raw {
            RawResponse::Success => Ok(Response::Success),
            RawResponse::AuthMessage {
                auth_message_type,
                auth_message,
            } => Ok(Response::AuthMessage {
                auth_message_type,
                auth_message,
            }),
            RawResponse::Error {
                description,
                error_type: _,
            } => Ok(Response::Error { description }),
        }
    }
}
