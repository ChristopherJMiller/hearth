//! Unix-socket IPC server for communication with the hearth-greeter.
//!
//! The protocol is newline-delimited JSON: each message is a single JSON
//! object terminated by `\n`. The greeter sends [`AgentRequest`]s and the
//! agent responds with [`AgentEvent`]s.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use hearth_common::api_client::HearthApiClient;
use hearth_common::api_types::UserEnvStatus;
use hearth_common::config::AgentConfig;
use hearth_common::ipc::{AgentEvent, AgentRequest};

/// Tracks the preparation status for each user.
#[derive(Debug, Clone)]
enum PrepareStatus {
    Preparing,
    Ready,
    Error(String),
}

/// Shared state across IPC connections.
struct IpcState<C> {
    prepare_status: HashMap<String, PrepareStatus>,
    client: Arc<C>,
    config: Arc<AgentConfig>,
    machine_id: Uuid,
}

/// Resolve a user's role from their group memberships using the configured mapping.
///
/// Returns the first matching role, or the default role if no mapping matches
/// (or `"default"` if no role mapping is configured at all).
fn resolve_role(groups: &[String], config: &AgentConfig) -> String {
    if let Some(mapping) = &config.role_mapping {
        for entry in &mapping.mappings {
            if groups.contains(&entry.group) {
                return entry.role.clone();
            }
        }
        mapping.default_role.clone()
    } else {
        "default".into()
    }
}

/// Run the IPC server on the given Unix socket path.
///
/// Listens for incoming connections, spawns a task per connection, and
/// shuts down cleanly when the cancellation token fires.
pub async fn run_ipc_server<C: HearthApiClient + 'static>(
    socket_path: &str,
    client: Arc<C>,
    config: Arc<AgentConfig>,
    machine_id: Uuid,
    shutdown: CancellationToken,
) {
    let path = Path::new(socket_path);

    // Clean up stale socket file if it exists.
    if path.exists()
        && let Err(e) = std::fs::remove_file(path)
    {
        error!(path = %socket_path, error = %e, "failed to remove stale socket file");
        return;
    }

    // Ensure the parent directory exists.
    if let Some(parent) = path.parent()
        && !parent.exists()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        error!(
            path = %parent.display(),
            error = %e,
            "failed to create socket parent directory"
        );
        return;
    }

    let listener = match UnixListener::bind(socket_path) {
        Ok(l) => l,
        Err(e) => {
            error!(path = %socket_path, error = %e, "failed to bind Unix socket");
            return;
        }
    };

    // Set socket permissions to 0o660 so the greeter group can connect.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o660);
        if let Err(e) = std::fs::set_permissions(socket_path, perms) {
            warn!(
                path = %socket_path,
                error = %e,
                "failed to set socket permissions"
            );
        }
    }

    info!(path = %socket_path, "IPC server listening");

    let state = Arc::new(Mutex::new(IpcState {
        prepare_status: HashMap::new(),
        client,
        config,
        machine_id,
    }));

    loop {
        tokio::select! {
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, _addr)) => {
                        debug!("accepted IPC connection");
                        let state = Arc::clone(&state);
                        let shutdown = shutdown.clone();
                        tokio::spawn(async move {
                            handle_connection(stream, state, shutdown).await;
                        });
                    }
                    Err(e) => {
                        warn!(error = %e, "failed to accept IPC connection");
                    }
                }
            }
            () = shutdown.cancelled() => {
                info!("IPC server shutting down");
                break;
            }
        }
    }

    // Clean up the socket file on shutdown.
    if let Err(e) = std::fs::remove_file(socket_path) {
        debug!(error = %e, "failed to remove socket file during shutdown (may already be gone)");
    }
}

/// Handle a single IPC connection.
///
/// Reads newline-delimited JSON requests, processes each one, and writes
/// back newline-delimited JSON responses.
async fn handle_connection<C: HearthApiClient + 'static>(
    stream: tokio::net::UnixStream,
    state: Arc<Mutex<IpcState<C>>>,
    shutdown: CancellationToken,
) {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    loop {
        let line = tokio::select! {
            result = lines.next_line() => {
                match result {
                    Ok(Some(line)) => line,
                    Ok(None) => {
                        debug!("IPC client disconnected");
                        return;
                    }
                    Err(e) => {
                        warn!(error = %e, "error reading from IPC client");
                        return;
                    }
                }
            }
            () = shutdown.cancelled() => {
                debug!("IPC connection closing due to shutdown");
                return;
            }
        };

        let request: AgentRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                warn!(error = %e, raw = %line, "invalid IPC request");
                let event = AgentEvent::Error {
                    username: String::new(),
                    message: format!("invalid request: {e}"),
                };
                if let Err(e) = send_event(&mut writer, &event).await {
                    warn!(error = %e, "failed to send error response");
                }
                continue;
            }
        };

        debug!(?request, "handling IPC request");

        match request {
            AgentRequest::Ping => {
                if let Err(e) = send_event(&mut writer, &AgentEvent::Pong).await {
                    warn!(error = %e, "failed to send Pong");
                    return;
                }
            }

            AgentRequest::PrepareUserEnv { username, groups } => {
                debug!(%username, ?groups, "preparing user environment");

                // Record that preparation has started.
                {
                    let mut st = state.lock().await;
                    st.prepare_status
                        .insert(username.clone(), PrepareStatus::Preparing);
                }

                // Send immediate "Preparing" acknowledgement.
                let preparing = AgentEvent::Preparing {
                    username: username.clone(),
                    message: "preparing user environment".into(),
                };
                if let Err(e) = send_event(&mut writer, &preparing).await {
                    warn!(error = %e, "failed to send Preparing event");
                    return;
                }

                // Spawn a background task to build/activate the user environment.
                let state_bg = Arc::clone(&state);
                let writer_shutdown = shutdown.clone();
                let user = username.clone();
                tokio::spawn(async move {
                    let role = {
                        let st = state_bg.lock().await;
                        resolve_role(&groups, &st.config)
                    };
                    info!(%user, %role, "resolved role for user");

                    // Extract client + machine_id so we don't hold the lock across awaits.
                    let (client, machine_id) = {
                        let st = state_bg.lock().await;
                        (Arc::clone(&st.client), st.machine_id)
                    };

                    // Report "building" status to control plane.
                    if let Err(e) = client
                        .report_user_env(machine_id, &user, &role, UserEnvStatus::Building)
                        .await
                    {
                        warn!(error = %e, "failed to report building status");
                    }

                    // Run home-manager activation if configured.
                    let flake_ref = {
                        let st = state_bg.lock().await;
                        st.config.home_flake_ref.clone()
                    };
                    let activation_result = {
                        if let Some(flake_ref) = flake_ref {
                            let flake_target = format!("{flake_ref}#{role}");
                            info!(%user, %flake_target, "activating home-manager environment");

                            tokio::select! {
                                output = tokio::process::Command::new("runuser")
                                    .args(["-u", &user, "--", "home-manager", "switch", "--flake", &flake_target])
                                    .output() => {
                                    match output {
                                        Ok(out) if out.status.success() => Ok(()),
                                        Ok(out) => {
                                            let stderr = String::from_utf8_lossy(&out.stderr);
                                            Err(format!("home-manager switch failed: {stderr}"))
                                        }
                                        Err(e) => Err(format!("failed to run home-manager: {e}")),
                                    }
                                }
                                () = writer_shutdown.cancelled() => {
                                    let mut st = state_bg.lock().await;
                                    st.prepare_status
                                        .insert(user.clone(), PrepareStatus::Error("shutdown during preparation".into()));
                                    return;
                                }
                            }
                        } else {
                            info!(%user, %role, "no home_flake_ref configured, skipping activation");
                            Ok(())
                        }
                    };

                    match activation_result {
                        Ok(()) => {
                            // Report active status + record login.
                            if let Err(e) = client
                                .report_user_env(machine_id, &user, &role, UserEnvStatus::Active)
                                .await
                            {
                                warn!(error = %e, "failed to report active status");
                            }
                            if let Err(e) = client.report_user_login(machine_id, &user).await {
                                warn!(error = %e, "failed to report user login");
                            }

                            let mut st = state_bg.lock().await;
                            st.prepare_status.insert(user, PrepareStatus::Ready);
                        }
                        Err(msg) => {
                            error!(%user, error = %msg, "user environment activation failed");
                            if let Err(e) = client
                                .report_user_env(machine_id, &user, &role, UserEnvStatus::Failed)
                                .await
                            {
                                warn!(error = %e, "failed to report failed status");
                            }

                            let mut st = state_bg.lock().await;
                            st.prepare_status.insert(user, PrepareStatus::Error(msg));
                        }
                    }
                });
            }

            AgentRequest::GetPrepareStatus { username } => {
                let st = state.lock().await;
                let event = match st.prepare_status.get(&username) {
                    Some(PrepareStatus::Preparing) => AgentEvent::Preparing {
                        username,
                        message: "still preparing".into(),
                    },
                    Some(PrepareStatus::Ready) => AgentEvent::Ready { username },
                    Some(PrepareStatus::Error(msg)) => AgentEvent::Error {
                        username,
                        message: msg.clone(),
                    },
                    None => AgentEvent::Error {
                        username,
                        message: "no preparation has been requested for this user".into(),
                    },
                };
                if let Err(e) = send_event(&mut writer, &event).await {
                    warn!(error = %e, "failed to send prepare status");
                    return;
                }
            }
        }
    }
}

/// Serialize an [`AgentEvent`] as a single JSON line and write it to the stream.
async fn send_event(
    writer: &mut tokio::net::unix::OwnedWriteHalf,
    event: &AgentEvent,
) -> std::io::Result<()> {
    let mut payload = serde_json::to_string(event)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    payload.push('\n');
    writer.write_all(payload.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}
