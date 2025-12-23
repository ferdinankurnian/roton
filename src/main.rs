use display_info::DisplayInfo;
use slint::PhysicalPosition;
use std::error::Error;
use std::sync::{Arc, Mutex};

mod recorder;
use recorder::Recorder;

slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {

    let app = AppWindow::new()?;
    let editor = EditorWindow::new()?;

    let app_weak = app.as_weak();
    let editor_weak = editor.as_weak();

    app.on_open_editor(move || {
        let app = app_weak.upgrade().unwrap();
        let editor = editor_weak.upgrade().unwrap();
        app.hide().unwrap();
        editor.show().unwrap();
    });

    let display_infos = DisplayInfo::all()?;
    if let Some(primary_display) = display_infos.iter().find(|d| d.is_primary) {
        let screen_width = primary_display.width;
        let screen_height = primary_display.height;

        let window = app.window();
        let window_size = window.size();

        // Calculate x for horizontal center
        let x = (screen_width as i32 - window_size.width as i32) / 2;
        // Calculate y for bottom alignment (minus 50px padding)
        let y = screen_height as i32 - window_size.height as i32 - 50;

        window.set_position(PhysicalPosition::new(x, y));
    }

    app.on_request_close({
        let app_weak = app.as_weak();
        move || {
            if let Some(app) = app_weak.upgrade() {
                app.hide().unwrap();
            }
        }
    });

    let recorder = Arc::new(Mutex::new(Recorder::new()));

    // Set initial save path
    if let Some(user_dirs) = directories::UserDirs::new() {
        if let Some(video_dir) = user_dirs.video_dir() {
            app.set_save_path(video_dir.to_string_lossy().to_string().into());
        } else {
            app.set_save_path(user_dirs.home_dir().to_string_lossy().to_string().into());
        }
    }

    app.on_choose_folder({
        let app_weak = app.as_weak();
        move || {
            if let Some(folder) = rfd::FileDialog::new()
                .set_title("Choose Save Folder")
                .pick_folder() {
                if let Some(app) = app_weak.upgrade() {
                    app.set_save_path(folder.to_string_lossy().to_string().into());
                }
            }
        }
    });

    // Check availability on startup
    if !Recorder::is_available() {
        eprintln!("wl-screenrec not found!");
    }

    app.on_start_recording({
        let recorder = recorder.clone();
        let app_weak = app.as_weak();
        move |mode, geometry| {
            let app = app_weak.upgrade().unwrap();
            let save_dir = app.get_save_path().to_string();
            let audio_mode = app.get_audio_mode().to_string();

            println!("Starting recording: mode={}, geometry={}, path={}, audio={}", 
                mode, geometry, save_dir, audio_mode);
            
            let filename = format!("recording_{}.mp4", chrono::Local::now().format("%Y-%m-%d_%H-%M-%S"));
            let path = std::path::Path::new(&save_dir).join(filename);
            
            let geo = if geometry.is_empty() { None } else { Some(geometry.as_str()) };

            if let Ok(mut rec) = recorder.lock() {
                if let Err(e) = rec.start_recording(path.to_str().unwrap(), geo, &audio_mode) {
                    eprintln!("Error starting recording: {}", e);
                }
            }
        }
    });

    app.on_stop_recording({
        let recorder = recorder.clone();
        move || {
            if let Ok(mut rec) = recorder.lock() {
                if let Err(e) = rec.stop_recording() {
                    eprintln!("Error stopping recording: {}", e);
                }
            }
        }
    });

    app.run()?;

    Ok(())
}
