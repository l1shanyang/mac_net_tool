mod app;
mod config;
mod net;
mod store;

use app::App;
use tray_icon::menu::MenuEvent;
use tray_icon::TrayIconEvent;
use winit::event::Event;
use winit::event_loop::EventLoopBuilder;

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
