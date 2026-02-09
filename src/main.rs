use eframe::egui;
use std::process::{Command, ExitStatus};

const SERVICE: &str = "Wi-Fi";
const IP: &str = "192.168.50.163";
const MASK: &str = "255.255.255.0";
const ROUTER: &str = "192.168.50.222";

fn run_cmd(cmd: &mut Command) -> Result<ExitStatus, String> {
    let status = cmd
        .status()
        .map_err(|e| format!("failed to run command: {e}"))?;
    Ok(status)
}

fn apply_config() -> Result<(), String> {
    let mut cmd = Command::new("networksetup");
    cmd.arg("-setmanual")
        .arg(SERVICE)
        .arg(IP)
        .arg(MASK)
        .arg(ROUTER);
    let status = run_cmd(&mut cmd)?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("command exited with status {status}"))
    }
}

fn stop_config() -> Result<(), String> {
    let mut cmd = Command::new("networksetup");
    cmd.arg("-setdhcp").arg(SERVICE);
    let status = run_cmd(&mut cmd)?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("command exited with status {status}"))
    }
}

struct NetApp {
    applied: bool,
    status: String,
}

impl Default for NetApp {
    fn default() -> Self {
        Self {
            applied: false,
            status: "Ready.".to_string(),
        }
    }
}

impl eframe::App for NetApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("macOS Network Config");
            ui.label(format!("Service: {SERVICE}"));
            ui.separator();

            ui.label(format!("IP: {IP}"));
            ui.label(format!("Mask: {MASK}"));
            ui.label(format!("Router: {ROUTER}"));
            ui.separator();

            let button_label = if self.applied { "Stop" } else { "Apply" };
            let clicked = ui.add_sized([160.0, 40.0], egui::Button::new(button_label)).clicked();

            if clicked {
                let result = if self.applied {
                    stop_config()
                } else {
                    apply_config()
                };
                match result {
                    Ok(()) => {
                        self.applied = !self.applied;
                        self.status = if self.applied {
                            "Applied manual IPv4 config.".to_string()
                        } else {
                            "Switched to DHCP.".to_string()
                        };
                    }
                    Err(e) => {
                        self.status = format!("Failed: {e}. Try running with sudo.");
                    }
                }
            }

            ui.separator();
            ui.label(&self.status);
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "macOS Network Config",
        options,
        Box::new(|_cc| Box::new(NetApp::default())),
    )
}
