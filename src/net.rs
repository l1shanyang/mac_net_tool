use rand::seq::SliceRandom;
use rand::thread_rng;
use std::process::{Command, ExitStatus};

use crate::config::{IP_BASE, MASK, ROUTER, SERVICE};
use crate::store::{load_last_ip, save_last_ip};

pub struct NetworkInfo {
    pub is_dhcp: bool,
    pub ip: Option<String>,
}

pub fn detect_network_state() -> Result<NetworkInfo, String> {
    let output = Command::new("networksetup")
        .arg("-getinfo")
        .arg(SERVICE)
        .output()
        .map_err(|e| format!("failed to run command: {e}"))?;
    if !output.status.success() {
        return Err(format!("command exited with status {}", output.status));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let is_dhcp = stdout.contains("DHCP Configuration") || stdout.contains("dhcp");
    let mut ip = None;
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("IP address:") {
            let val = rest.trim();
            if !val.is_empty() {
                ip = Some(val.to_string());
            }
        }
    }
    Ok(NetworkInfo { is_dhcp, ip })
}

pub fn apply_config() -> Result<String, String> {
    let ip = match load_last_ip()? {
        Some(ip) if ip.starts_with(&format!("{IP_BASE}.")) && !ip_in_use(&ip)? => ip,
        _ => choose_free_ip()?,
    };
    let mut cmd = Command::new("networksetup");
    cmd.arg("-setmanual")
        .arg(SERVICE)
        .arg(&ip)
        .arg(MASK)
        .arg(ROUTER);
    let status = run_cmd(&mut cmd)?;
    if status.success() {
        let _ = save_last_ip(&ip);
        Ok(ip)
    } else {
        Err(format!("command exited with status {status}"))
    }
}

pub fn stop_config() -> Result<(), String> {
    let mut cmd = Command::new("networksetup");
    cmd.arg("-setdhcp").arg(SERVICE);
    let status = run_cmd(&mut cmd)?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("command exited with status {status}"))
    }
}

fn choose_free_ip() -> Result<String, String> {
    let router_last = ROUTER
        .split('.')
        .last()
        .and_then(|s| s.parse::<u8>().ok())
        .unwrap_or(222);

    let mut candidates: Vec<u8> = (2..=254)
        .filter(|&n| n != router_last && n != 1)
        .collect();
    candidates.shuffle(&mut thread_rng());

    for last in candidates.into_iter().take(100) {
        let ip = format!("{IP_BASE}.{last}");
        if !ip_in_use(&ip)? {
            return Ok(ip);
        }
    }

    Err("no available IP found in subnet".to_string())
}

fn ip_in_use(ip: &str) -> Result<bool, String> {
    let status = Command::new("ping")
        .arg("-c")
        .arg("1")
        .arg("-W")
        .arg("1000")
        .arg(ip)
        .status()
        .map_err(|e| format!("failed to run ping: {e}"))?;
    Ok(status.success())
}

fn run_cmd(cmd: &mut Command) -> Result<ExitStatus, String> {
    let status = cmd
        .status()
        .map_err(|e| format!("failed to run command: {e}"))?;
    Ok(status)
}
