//! hearth-agent: the on-device agent for the Hearth platform.
//!
//! Orchestrates:
//! - Polling the control plane for target state and sending heartbeats.
//! - An IPC server for the greeter to request user-environment preparation.
//! - Coordinated shutdown on SIGTERM / SIGINT.

mod actions;
mod config;
mod headscale;
mod installer;
mod ipc;
mod metrics;
mod poller;
mod queue;
mod updater;

use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use uuid::Uuid;

use std::os::unix::net::UnixDatagram;

use hearth_common::api_client::ReqwestApiClient;

fn main() {
    // Consume systemd socket activation env vars before starting the async
    // runtime, since env::remove_var is unsafe in multi-threaded contexts.
    ipc::consume_listen_fds();

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime")
        .block_on(async_main());
}

async fn async_main() {
    // Initialise structured logging (JSON when LOG_FORMAT=json).
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "hearth_agent=info".into());

    if std::env::var("LOG_FORMAT").as_deref() == Ok("json") {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(env_filter)
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    }

    info!("hearth-agent starting");

    // --- Load configuration ---
    let args: Vec<String> = std::env::args().collect();
    let config_path = config::resolve_config_path(&args);
    info!(path = %config_path.display(), "loading configuration");

    let cfg = match config::load_config(&config_path) {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "failed to load configuration");
            std::process::exit(1);
        }
    };

    // --- Resolve machine identity ---
    let machine_id: Uuid = match &cfg.server.machine_id {
        Some(id_str) => match id_str.parse() {
            Ok(id) => id,
            Err(e) => {
                error!(
                    machine_id = %id_str,
                    error = %e,
                    "invalid machine_id in config, must be a valid UUID"
                );
                std::process::exit(1);
            }
        },
        None => {
            // Try reading the machine-id file written during enrollment.
            let machine_id_path = std::path::Path::new("/var/lib/hearth/machine-id");
            match std::fs::read_to_string(machine_id_path) {
                Ok(contents) => match contents.trim().parse() {
                    Ok(id) => {
                        info!(%id, "loaded machine_id from {}", machine_id_path.display());
                        id
                    }
                    Err(e) => {
                        error!(
                            error = %e,
                            path = %machine_id_path.display(),
                            "machine-id file exists but is not a valid UUID"
                        );
                        std::process::exit(1);
                    }
                },
                Err(_) => {
                    let id = Uuid::new_v4();
                    warn!(
                        %id,
                        "no machine_id in config or file, generated a random one (dev mode)"
                    );
                    id
                }
            }
        }
    };

    // When a mesh server URL is configured, use it as the primary API endpoint.
    // This enables communication over the Headscale mesh for environments where
    // the control plane isn't reachable over the public internet.
    let api_url = cfg
        .headscale
        .as_ref()
        .and_then(|hs| hs.mesh_server_url.as_deref())
        .unwrap_or(&cfg.server.url);

    info!(%machine_id, server = %api_url, "agent configured");

    // --- Build shared API client (with machine token if available) ---
    let client = {
        let token_path = std::path::Path::new(&cfg.agent.machine_token_path);
        match std::fs::read_to_string(token_path) {
            Ok(token) => {
                let token = token.trim().to_string();
                info!("loaded machine token from {}", token_path.display());
                Arc::new(ReqwestApiClient::new_with_token(api_url.to_string(), token))
            }
            Err(_) => {
                warn!(
                    path = %token_path.display(),
                    "no machine token found, running without auth (dev mode)"
                );
                Arc::new(ReqwestApiClient::new(api_url.to_string()))
            }
        }
    };

    // --- Coordinated shutdown ---
    let shutdown = CancellationToken::new();

    // --- Open offline queue ---
    let queue_path = std::path::PathBuf::from(&cfg.agent.queue_path);
    let offline_queue = match queue::OfflineQueue::open(&queue_path) {
        Ok(q) => Arc::new(q),
        Err(e) => {
            error!(error = %e, path = %queue_path.display(), "failed to open offline queue");
            std::process::exit(1);
        }
    };

    // Spawn the poll loop.
    let poll_shutdown = shutdown.clone();
    let poll_client = Arc::clone(&client);
    let poll_interval = Duration::from_secs(cfg.agent.poll_interval_secs);
    let poll_queue = Arc::clone(&offline_queue);
    let poll_token_path = std::path::PathBuf::from(&cfg.agent.machine_token_path);
    let poll_handle = tokio::spawn(async move {
        poller::run_poll_loop(
            poll_client,
            machine_id,
            poll_interval,
            poll_queue,
            poll_token_path,
            poll_shutdown,
        )
        .await;
    });

    // Spawn the IPC server. Wait for it to signal readiness before
    // notifying systemd, so the socket is guaranteed to be listening.
    let ipc_shutdown = shutdown.clone();
    let ipc_client = Arc::clone(&client);
    let ipc_config = Arc::new(cfg.clone());
    let socket_path = cfg.agent.socket_path.clone();
    let (ipc_ready_tx, ipc_ready_rx) = tokio::sync::oneshot::channel();
    let ipc_handle = tokio::spawn(async move {
        ipc::run_ipc_server(
            &socket_path,
            ipc_client,
            ipc_config,
            machine_id,
            ipc_shutdown,
            Some(ipc_ready_tx),
        )
        .await;
    });

    // Wait for IPC socket to be ready, then notify systemd.
    let _ = ipc_ready_rx.await;
    notify_ready();

    // Spawn the signal handler.
    let sig_shutdown = shutdown.clone();
    let signal_handle = tokio::spawn(async move {
        wait_for_shutdown_signal().await;
        info!("shutdown signal received, stopping all tasks");
        sig_shutdown.cancel();
    });

    // Wait for any task to complete (the signal handler will be first in
    // normal operation), then ensure the others wind down.
    tokio::select! {
        _ = poll_handle => {
            warn!("poll loop exited unexpectedly");
            shutdown.cancel();
        }
        _ = ipc_handle => {
            warn!("IPC server exited unexpectedly");
            shutdown.cancel();
        }
        _ = signal_handle => {
            // Signal handler completed normally; shutdown is already
            // triggered via the cancellation token.
        }
    }

    // Give tasks a moment to finish cleanly.
    tokio::time::sleep(Duration::from_millis(250)).await;

    info!("hearth-agent stopped");
}

/// Notify systemd that the service is ready (sd_notify protocol).
fn notify_ready() {
    let socket_path = match std::env::var("NOTIFY_SOCKET") {
        Ok(p) => p,
        Err(_) => return, // Not running under systemd Type=notify
    };

    let sock = match UnixDatagram::unbound() {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "failed to create notify socket");
            return;
        }
    };

    if let Err(e) = sock.send_to(b"READY=1", &socket_path) {
        warn!(error = %e, "failed to send sd_notify READY=1");
    } else {
        info!("notified systemd: READY=1");
    }
}

/// Wait for SIGTERM or SIGINT (Ctrl-C).
async fn wait_for_shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};

    let mut sigterm = signal(SignalKind::terminate()).expect("failed to register SIGTERM handler");
    let mut sigint = signal(SignalKind::interrupt()).expect("failed to register SIGINT handler");

    tokio::select! {
        _ = sigterm.recv() => {
            info!("received SIGTERM");
        }
        _ = sigint.recv() => {
            info!("received SIGINT");
        }
    }
}
