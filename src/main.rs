use std::process::{Command, ExitStatus};

use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use winit::event::Event;
use winit::event_loop::EventLoopBuilder;

const SERVICE: &str = "Wi-Fi";
const IP: &str = "192.168.50.163";
const MASK: &str = "255.255.255.0";
const ROUTER: &str = "192.168.50.222";

struct App {
    tray: Option<TrayIcon>,
    menu: Option<Menu>,
    toggle_item: Option<MenuItem>,
    quit_item: Option<MenuItem>,
    applied: bool,
    status: String,
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
        }
    }

    fn init(&mut self) {
        if let Ok(is_dhcp) = detect_dhcp() {
            self.applied = !is_dhcp;
            self.status = if is_dhcp {
                "DHCP".to_string()
            } else {
                "PROXY".to_string()
            };
        } else {
            self.applied = false;
            self.status = "Unknown".to_string();
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
            apply_config()
        };

        match result {
            Ok(()) => {
                self.applied = !self.applied;
                self.status = if self.applied { "PROXY".to_string() } else { "DHCP".to_string() };
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
            let _ = tray.set_tooltip(Some(&self.status));
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

fn detect_dhcp() -> Result<bool, String> {
    let output = Command::new("networksetup")
        .arg("-getinfo")
        .arg(SERVICE)
        .output()
        .map_err(|e| format!("failed to run command: {e}"))?;
    if !output.status.success() {
        return Err(format!("command exited with status {}", output.status));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.contains("DHCP Configuration") || stdout.contains("dhcp"))
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
