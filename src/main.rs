use std::fs;
use std::path::PathBuf;
use std::process::{Command, ExitStatus};

use rand::seq::SliceRandom;
use rand::thread_rng;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use winit::event::Event;
use winit::event_loop::EventLoopBuilder;

const SERVICE: &str = "Wi-Fi";
const IP_BASE: &str = "192.168.50";
const MASK: &str = "255.255.255.0";
const ROUTER: &str = "192.168.50.222";

struct App {
    tray: Option<TrayIcon>,
    menu: Option<Menu>,
    toggle_item: Option<MenuItem>,
    quit_item: Option<MenuItem>,
    applied: bool,
    status: String,
    current_ip: Option<String>,
}

impl App {
    fn new() -> Self {
        Self {
            tray: None,
            menu: None,
            toggle_item: None,
            quit_item: None,
            applied: false,
            status: "Ready.".to_string(),
            current_ip: None,
        }
    }

    fn init(&mut self) {
        if let Ok(info) = detect_network_state() {
            self.applied = !info.is_dhcp;
            self.status = if info.is_dhcp { "DHCP" } else { "PROXY" }.to_string();
            self.current_ip = info.ip;
        } else {
            self.applied = false;
            self.status = "Unknown".to_string();
            self.current_ip = None;
        }

        #[cfg(target_os = "macos")]
        unsafe {
            use cocoa::appkit::{NSApp, NSApplication, NSApplicationActivationPolicy};
            let app = NSApp();
            app.setActivationPolicy_(
                NSApplicationActivationPolicy::NSApplicationActivationPolicyProhibited,
            );
        }

        let toggle_item = MenuItem::new("Apply", true, None);
        let quit_item = MenuItem::new("Quit", true, None);

        let menu = Menu::new();
        let _ = menu.append(&toggle_item);
        let _ = menu.append(&quit_item);

        let icon = make_icon(self.applied);

        let tray = TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip("macOS Network Config")
            .with_title(if self.applied { "PROXY" } else { "DHCP" })
            .with_menu(Box::new(menu.clone()))
            .with_menu_on_left_click(false)
            .build()
            .ok();

        self.tray = tray;
        self.menu = Some(menu);
        self.toggle_item = Some(toggle_item);
        self.quit_item = Some(quit_item);
        self.update_tray();
    }

    fn on_tray_event(&mut self, event: TrayIconEvent) {
        if let TrayIconEvent::Click {
            button,
            button_state,
            ..
        } = event
        {
            if button == MouseButton::Left && button_state == MouseButtonState::Up {
                let _ = self.toggle();
            }
        }
    }

    fn on_menu_event(&mut self, event: MenuEvent) -> bool {
        if let Some(item) = &self.toggle_item {
            if event.id == *item.id() {
                let _ = self.toggle();
                return false;
            }
        }
        if let Some(item) = &self.quit_item {
            if event.id == *item.id() {
                return true;
            }
        }
        false
    }

    fn toggle(&mut self) -> Result<(), String> {
        let result = if self.applied {
            stop_config()
        } else {
            apply_config().map(|ip| {
                self.current_ip = Some(ip);
            })
        };

        match result {
            Ok(()) => {
                self.applied = !self.applied;
                self.status = if self.applied { "PROXY".to_string() } else { "DHCP".to_string() };
                if !self.applied {
                    self.current_ip = None;
                }
                self.update_tray();
                Ok(())
            }
            Err(e) => {
                self.status = format!("Failed: {e}. Try running with sudo.");
                self.update_tray();
                Err(e)
            }
        }
    }

    fn update_tray(&mut self) {
        if let Some(tray) = &self.tray {
            let _ = tray.set_icon(Some(make_icon(self.applied)));
            tray.set_title(Some(if self.applied { "PROXY" } else { "DHCP" }));
            let tip = if let Some(ip) = &self.current_ip {
                format!("{} ({ip})", self.status)
            } else {
                self.status.clone()
            };
            let _ = tray.set_tooltip(Some(&tip));
        }
        if let Some(item) = &self.toggle_item {
            item.set_text(if self.applied { "Stop" } else { "Apply" });
        }
    }
}

fn make_icon(applied: bool) -> Icon {
    let size = 18u32;
    let color = if applied {
        [46, 204, 113, 255]
    } else {
        [149, 165, 166, 255]
    };
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for _ in 0..(size * size) {
        rgba.extend_from_slice(&color);
    }
    Icon::from_rgba(rgba, size, size).expect("icon")
}

fn run_cmd(cmd: &mut Command) -> Result<ExitStatus, String> {
    let status = cmd
        .status()
        .map_err(|e| format!("failed to run command: {e}"))?;
    Ok(status)
}

struct NetworkInfo {
    is_dhcp: bool,
    ip: Option<String>,
}

fn detect_network_state() -> Result<NetworkInfo, String> {
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

fn apply_config() -> Result<String, String> {
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

fn state_file_path() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|e| format!("failed to read HOME: {e}"))?;
    Ok(PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("MacNetConfig")
        .join("last_ip.txt"))
}

fn load_last_ip() -> Result<Option<String>, String> {
    let path = state_file_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path).map_err(|e| format!("read last_ip: {e}"))?;
    let ip = content.trim().to_string();
    if ip.is_empty() {
        Ok(None)
    } else {
        Ok(Some(ip))
    }
}

fn save_last_ip(ip: &str) -> Result<(), String> {
    let path = state_file_path()?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("create state dir: {e}"))?;
    }
    fs::write(path, ip).map_err(|e| format!("write last_ip: {e}"))?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoopBuilder::<()>::with_user_event().build()?;
    let mut app = App::new();

    event_loop.run(move |event, elwt| {
        if let Event::NewEvents(winit::event::StartCause::Init) = event {
            app.init();
            return;
        }

        if let Event::AboutToWait = event {
            while let Ok(e) = TrayIconEvent::receiver().try_recv() {
                app.on_tray_event(e);
            }
            while let Ok(e) = MenuEvent::receiver().try_recv() {
                if app.on_menu_event(e) {
                    elwt.exit();
                    break;
                }
            }
        }
    })?;

    Ok(())
}
