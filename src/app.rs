use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::net::{apply_config, detect_network_state, stop_config};

pub struct App {
    tray: Option<TrayIcon>,
    menu: Option<Menu>,
    toggle_item: Option<MenuItem>,
    quit_item: Option<MenuItem>,
    applied: bool,
    status: String,
    current_ip: Option<String>,
}

impl App {
    pub fn new() -> Self {
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

    pub fn init(&mut self) {
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
    }

    pub fn on_tray_event(&mut self, event: TrayIconEvent) {
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

    pub fn on_menu_event(&mut self, event: MenuEvent) -> bool {
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
