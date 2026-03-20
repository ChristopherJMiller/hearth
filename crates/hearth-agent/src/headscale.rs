//! Headscale mesh VPN utilities for the agent.

use tracing::debug;

/// Detect the local Headscale/Tailscale IP address by querying `tailscale status`.
///
/// Returns the first IPv4 address from the Tailscale interface, or `None` if
/// Tailscale is not running or not connected.
pub fn detect_headscale_ip() -> Option<String> {
    let output = std::process::Command::new("tailscale")
        .args(["status", "--json"])
        .output()
        .ok()?;

    if !output.status.success() {
        debug!("tailscale status failed, mesh VPN not available");
        return None;
    }

    let status: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;

    // Extract the first IPv4 address from Self.TailscaleIPs
    let ips = status.get("Self")?.get("TailscaleIPs")?.as_array()?;
    for ip in ips {
        let ip_str = ip.as_str()?;
        // Prefer IPv4 (100.x.y.z CGNAT range)
        if !ip_str.contains(':') {
            debug!(headscale_ip = ip_str, "detected Headscale mesh IP");
            return Some(ip_str.to_string());
        }
    }

    // Fall back to first IP if no IPv4 found
    ips.first().and_then(|v| v.as_str()).map(|s| s.to_string())
}
