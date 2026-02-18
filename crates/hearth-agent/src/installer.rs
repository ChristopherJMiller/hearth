//! Software install executor: runs Flatpak installs per-user, stubs for Nix methods.

use hearth_common::api_types::{InstallMethod, PendingSoftwareInstall};

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("install method {0:?} is not yet supported")]
    Unsupported(InstallMethod),
    #[error("flatpak install failed: {0}")]
    FlatpakFailed(String),
    #[error("missing flatpak_ref for flatpak install")]
    MissingFlatpakRef,
}

/// Execute a software install for the given user.
pub async fn execute_install(install: &PendingSoftwareInstall) -> Result<(), InstallError> {
    let method = install.catalog_entry.install_method;
    match method {
        InstallMethod::Flatpak => install_flatpak(install).await,
        other => {
            tracing::warn!(?other, name = %install.catalog_entry.name, "install method not yet implemented");
            Err(InstallError::Unsupported(other))
        }
    }
}

async fn install_flatpak(install: &PendingSoftwareInstall) -> Result<(), InstallError> {
    let flatpak_ref = install
        .catalog_entry
        .flatpak_ref
        .as_deref()
        .ok_or(InstallError::MissingFlatpakRef)?;

    let username = &install.username;

    tracing::info!(
        %username,
        %flatpak_ref,
        name = %install.catalog_entry.name,
        "installing flatpak for user"
    );

    let output = tokio::process::Command::new("runuser")
        .args([
            "-u",
            username,
            "--",
            "flatpak",
            "install",
            "--user",
            "--noninteractive",
            flatpak_ref,
        ])
        .output()
        .await
        .map_err(|e| InstallError::FlatpakFailed(e.to_string()))?;

    if output.status.success() {
        tracing::info!(%flatpak_ref, %username, "flatpak install succeeded");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!(%flatpak_ref, %username, %stderr, "flatpak install failed");
        Err(InstallError::FlatpakFailed(stderr.into_owned()))
    }
}
