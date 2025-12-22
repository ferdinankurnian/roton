// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use display_info::DisplayInfo;
use slint::PhysicalPosition;
use std::error::Error;

slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {

    let ui = AppWindow::new()?;

    let display_infos = DisplayInfo::all()?;
    if let Some(primary_display) = display_infos.iter().find(|d| d.is_primary) {
        let screen_width = primary_display.width;
        let screen_height = primary_display.height;

        let window = ui.window();
        let window_size = window.size();

        // Calculate x for horizontal center
        let x = (screen_width as i32 - window_size.width as i32) / 2;
        // Calculate y for bottom alignment (minus 50px padding)
        let y = screen_height as i32 - window_size.height as i32 - 50;

        window.set_position(PhysicalPosition::new(x, y));
    }

    ui.on_request_close({
        let ui_handle = ui.as_weak();
        move || {
            if let Some(ui) = ui_handle.upgrade() {
                ui.hide().unwrap();
            }
        }
    });

    ui.run()?;

    Ok(())
}
