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
use tokio::sync::{Mutex, mpsc};
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
/// back newline-delimited JSON responses. Background tasks (e.g. environment
/// preparation) send events back through an mpsc channel that is multiplexed
/// with the incoming request stream.
async fn handle_connection<C: HearthApiClient + 'static>(
    stream: tokio::net::UnixStream,
    state: Arc<Mutex<IpcState<C>>>,
    shutdown: CancellationToken,
) {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    // Channel for background tasks to send events back to this connection.
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(32);

    loop {
        tokio::select! {
            // Forward events from background tasks to the client.
            Some(event) = event_rx.recv() => {
                if let Err(e) = send_event(&mut writer, &event).await {
                    warn!(error = %e, "failed to forward background event to client");
                    return;
                }
            }

            // Read incoming requests from the client.
            result = lines.next_line() => {
                let line = match result {
                    Ok(Some(line)) => line,
                    Ok(None) => {
                        debug!("IPC client disconnected");
                        return;
                    }
                    Err(e) => {
                        warn!(error = %e, "error reading from IPC client");
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
                        handle_prepare_user_env(
                            &username,
                            groups,
                            &state,
                            &mut writer,
                            &event_tx,
                            &shutdown,
                        )
                        .await;
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

            () = shutdown.cancelled() => {
                debug!("IPC connection closing due to shutdown");
                return;
            }
        }
    }
}

/// Handle a `PrepareUserEnv` request: send an immediate acknowledgement, then
/// spawn a background task that performs the actual activation and streams
/// progress events back through `event_tx`.
async fn handle_prepare_user_env<C: HearthApiClient + 'static>(
    username: &str,
    groups: Vec<String>,
    state: &Arc<Mutex<IpcState<C>>>,
    writer: &mut tokio::net::unix::OwnedWriteHalf,
    event_tx: &mpsc::Sender<AgentEvent>,
    shutdown: &CancellationToken,
) {
    debug!(%username, ?groups, "preparing user environment");

    // Record that preparation has started.
    {
        let mut st = state.lock().await;
        st.prepare_status
            .insert(username.to_string(), PrepareStatus::Preparing);
    }

    // Send immediate "Preparing" acknowledgement.
    let preparing = AgentEvent::Preparing {
        username: username.to_string(),
        message: "preparing user environment".into(),
    };
    if let Err(e) = send_event(writer, &preparing).await {
        warn!(error = %e, "failed to send Preparing event");
        return;
    }

    // Spawn a background task to build/activate the user environment.
    let state_bg = Arc::clone(state);
    let bg_shutdown = shutdown.clone();
    let event_tx = event_tx.clone();
    let user = username.to_string();
    tokio::spawn(async move {
        // Single lock acquisition to extract everything we need.
        let (role, client, machine_id, flake_ref) = {
            let st = state_bg.lock().await;
            let role = resolve_role(&groups, &st.config);
            let client = Arc::clone(&st.client);
            let machine_id = st.machine_id;
            let flake_ref = st.config.home.as_ref().map(|h| h.flake_ref.clone());
            (role, client, machine_id, flake_ref)
        };
        info!(%user, %role, "resolved role for user");

        // Send progress: preparing.
        let _ = event_tx
            .send(AgentEvent::Progress {
                username: user.clone(),
                percent: 10,
                message: format!("preparing {role} environment"),
            })
            .await;

        // Report "building" status to control plane.
        if let Err(e) = client
            .report_user_env(machine_id, &user, &role, UserEnvStatus::Building)
            .await
        {
            warn!(error = %e, "failed to report building status");
        }

        // Try to use a pre-built per-user closure from the control plane.
        // If available, pull it from the cache and activate it.
        // If not, fall back to role template activation via home-manager switch.
        let activation_result = {
            // Step 1: Check for pre-built closure.
            let prebuilt = match client.get_user_env_closure(&user).await {
                Ok(resp) => resp,
                Err(e) => {
                    debug!(error = %e, "failed to query per-user closure, falling back to role template");
                    hearth_common::api_types::UserEnvClosureResponse {
                        closure: None,
                        cache_url: None,
                        fallback_role: role.clone(),
                    }
                }
            };

            if let Some(closure) = &prebuilt.closure {
                // Pre-built per-user closure available — pull and activate.
                info!(%user, %closure, "activating pre-built per-user closure");

                let _ = event_tx
                    .send(AgentEvent::Progress {
                        username: user.clone(),
                        percent: 30,
                        message: "pulling pre-built environment from cache".into(),
                    })
                    .await;

                // Pull closure from binary cache if a cache URL is provided.
                if let Some(cache_url) = &prebuilt.cache_url {
                    let copy_result = tokio::process::Command::new("nix")
                        .args(["copy", "--from", cache_url, closure])
                        .output()
                        .await;
                    if let Err(e) = &copy_result {
                        warn!(error = %e, "nix copy from cache failed, closure may already be local");
                    } else if let Ok(out) = &copy_result
                        && !out.status.success()
                    {
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        warn!(%stderr, "nix copy returned non-zero, continuing anyway");
                    }
                }

                let _ = event_tx
                    .send(AgentEvent::Progress {
                        username: user.clone(),
                        percent: 60,
                        message: "activating per-user environment".into(),
                    })
                    .await;

                // Activate the home-manager generation.
                let activate_path = format!("{closure}/activate");
                tokio::select! {
                    output = tokio::process::Command::new("runuser")
                        .args(["-u", &user, "--", &activate_path])
                        .output() => {
                        match output {
                            Ok(out) if out.status.success() => Ok(()),
                            Ok(out) => {
                                let stderr = String::from_utf8_lossy(&out.stderr);
                                Err(format!("per-user closure activation failed: {stderr}"))
                            }
                            Err(e) => Err(format!("failed to activate per-user closure: {e}")),
                        }
                    }
                    () = bg_shutdown.cancelled() => {
                        let mut st = state_bg.lock().await;
                        st.prepare_status
                            .insert(user.clone(), PrepareStatus::Error("shutdown during preparation".into()));
                        let _ = event_tx
                            .send(AgentEvent::Error {
                                username: user,
                                message: "shutdown during preparation".into(),
                            })
                            .await;
                        return;
                    }
                }
            } else if let Some(flake_ref) = flake_ref {
                // No pre-built closure — fall back to role template via home-manager switch.
                let flake_target = format!("{flake_ref}#{role}");
                info!(%user, %flake_target, "no pre-built closure, falling back to role template");

                let _ = event_tx
                    .send(AgentEvent::Progress {
                        username: user.clone(),
                        percent: 30,
                        message: format!("activating role template: {role}"),
                    })
                    .await;

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
                    () = bg_shutdown.cancelled() => {
                        let mut st = state_bg.lock().await;
                        st.prepare_status
                            .insert(user.clone(), PrepareStatus::Error("shutdown during preparation".into()));
                        let _ = event_tx
                            .send(AgentEvent::Error {
                                username: user,
                                message: "shutdown during preparation".into(),
                            })
                            .await;
                        return;
                    }
                }
            } else {
                info!(%user, %role, "no pre-built closure and no home_flake_ref configured, skipping activation");
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
                st.prepare_status.insert(user.clone(), PrepareStatus::Ready);

                let _ = event_tx.send(AgentEvent::Ready { username: user }).await;
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
                st.prepare_status
                    .insert(user.clone(), PrepareStatus::Error(msg.clone()));

                let _ = event_tx
                    .send(AgentEvent::Error {
                        username: user,
                        message: msg,
                    })
                    .await;
            }
        }
    });
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
