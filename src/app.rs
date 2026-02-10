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

        let icon = load_icon(self.applied);

        let tray = TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip("macOS Network Config")
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
            let _ = tray.set_icon(Some(load_icon(self.applied)));
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

fn load_icon(applied: bool) -> Icon {
    let bytes = include_bytes!("assets/logo.png");
    let img = image::load_from_memory(bytes).expect("load icon");
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let mut data = rgba.into_raw();

    if !applied {
        for px in data.chunks_exact_mut(4) {
            let r = px[0] as f32;
            let g = px[1] as f32;
            let b = px[2] as f32;
            let gray = (0.299 * r + 0.587 * g + 0.114 * b) as u8;
            px[0] = gray;
            px[1] = gray;
            px[2] = gray;
        }
    }

    Icon::from_rgba(data, width, height).expect("icon")
}
