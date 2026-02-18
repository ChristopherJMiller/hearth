use std::process::Command;

use crate::app::EnrollmentData;

pub fn detect_hostname() -> String {
    Command::new("hostname")
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if name.is_empty() { None } else { Some(name) }
            } else {
                None
            }
        })
        .or_else(|| {
            std::fs::read_to_string("/etc/hostname")
                .ok()
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "unknown".into())
}

pub fn detect_cpu() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|content| {
            content
                .lines()
                .find(|line| line.starts_with("model name"))
                .map(|line| {
                    line.split(':')
                        .nth(1)
                        .unwrap_or("unknown")
                        .trim()
                        .to_string()
                })
        })
        .unwrap_or_else(|| "unknown".into())
}

pub fn detect_ram() -> String {
    std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|content| {
            content
                .lines()
                .find(|line| line.starts_with("MemTotal"))
                .map(|line| {
                    let kb: u64 = line
                        .split_whitespace()
                        .nth(1)
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                    let gb = kb as f64 / 1_048_576.0;
                    format!("{gb:.1} GB")
                })
        })
        .unwrap_or_else(|| "unknown".into())
}

pub fn detect_disk() -> String {
    Command::new("lsblk")
        .args(["-nd", "-o", "NAME,SIZE", "-e", "7,11"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                let first_line = text.lines().next()?.trim().to_string();
                if first_line.is_empty() {
                    None
                } else {
                    Some(first_line)
                }
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".into())
}

pub fn detect_nic() -> String {
    Command::new("ip")
        .args(["-o", "link", "show"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                // Find first non-loopback interface
                text.lines()
                    .find(|line| !line.contains("LOOPBACK"))
                    .and_then(|line| {
                        // Format: "2: enp0s3: <BROADCAST..."
                        line.split(':').nth(1).map(|s| s.trim().to_string())
                    })
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".into())
}

pub fn detect_ip() -> String {
    Command::new("ip")
        .args(["-4", "-o", "addr", "show", "scope", "global"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                text.lines().next().and_then(|line| {
                    // Format: "2: enp0s3    inet 10.0.2.15/24 ..."
                    line.split_whitespace()
                        .nth(3)
                        .map(|addr| addr.split('/').next().unwrap_or(addr).to_string())
                })
            } else {
                None
            }
        })
        .unwrap_or_else(|| "no address".into())
}

pub fn hardware_fingerprint() -> String {
    // Try dmidecode for system UUID, fall back to machine-id
    Command::new("dmidecode")
        .args(["-s", "system-uuid"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                let uuid = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if uuid.is_empty() || uuid.contains("Not") {
                    None
                } else {
                    Some(uuid)
                }
            } else {
                None
            }
        })
        .or_else(|| {
            std::fs::read_to_string("/etc/machine-id")
                .ok()
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "unknown".into())
}

pub fn detect_all(data: &mut EnrollmentData) {
    data.hostname = detect_hostname();
    data.cpu = detect_cpu();
    data.ram = detect_ram();
    data.disk = detect_disk();
    data.nic = detect_nic();
    data.ip_address = detect_ip();
    data.hardware_fingerprint = Some(hardware_fingerprint());
}
