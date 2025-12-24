
use std::error::Error;
use std::sync::{Arc, Mutex};

mod recorder;
mod config;

use recorder::Recorder;
use config::Settings;

slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {

    let app = AppWindow::new()?;
    let last_path = Arc::new(Mutex::new(None));

    app.on_request_close({
        let app_weak = app.as_weak();
        move || {
            if let Some(app) = app_weak.upgrade() {
                app.hide().unwrap();
            }
        }
    });

    let recorder = Arc::new(Mutex::new(Recorder::new()));

    // Load persisted settings
    let settings = Settings::load();
    app.set_save_path(settings.save_path.into());
    app.set_audio_mode(settings.audio_mode.into());

    app.on_choose_folder({
        let app_weak = app.as_weak();
        move || {
            if let Some(folder) = rfd::FileDialog::new()
                .set_title("Choose Save Folder")
                .pick_folder() {
                if let Some(app) = app_weak.upgrade() {
                    let path = folder.to_string_lossy().to_string();
                    app.set_save_path(path.clone().into());
                    
                    // Save new path
                    let mut settings = Settings::load();
                    settings.save_path = path;
                    if let Err(e) = settings.save() {
                        eprintln!("Error saving settings: {}", e);
                    }
                }
            }
        }
    });
    
    app.on_audio_mode_changed({
        move |mode| {
            let mut settings = Settings::load();
            settings.audio_mode = mode.to_string();
            if let Err(e) = settings.save() {
                eprintln!("Error saving settings: {}", e);
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
        let last_path = last_path.clone();
        move |mode, geometry| {
            let app = app_weak.upgrade().unwrap();
            let save_dir = app.get_save_path().to_string();
            let audio_mode = app.get_audio_mode().to_string();

            println!("Starting recording: mode={}, geometry={}, path={}, audio={}", 
                mode, geometry, save_dir, audio_mode);
            
            let filename = format!("recording_{}.mp4", chrono::Local::now().format("%Y-%m-%d_%H-%M-%S"));
            let path = std::path::Path::new(&save_dir).join(filename);
            let path_str = path.to_str().unwrap().to_string();
            
            // Store path for thumbnail generation
            if let Ok(mut last) = last_path.lock() {
                *last = Some(path_str.clone());
            }

            let geo = if geometry.is_empty() { None } else { Some(geometry.as_str()) };

            if let Ok(mut rec) = recorder.lock() {
                // Save settings (including current audio mode) when starting recording
                let mut current_settings = Settings::load();
                current_settings.save_path = save_dir.clone();
                current_settings.audio_mode = audio_mode.clone();
                let _ = current_settings.save();

                if let Err(e) = rec.start_recording(&path_str, geo, &audio_mode) {
                    eprintln!("Error starting recording: {}", e);
                }
            }
        }
    });

    app.on_stop_recording({
        let recorder = recorder.clone();
        let app_weak = app.as_weak();
        let last_path = last_path.clone();
        move || {
            if let Ok(mut rec) = recorder.lock() {
                if let Err(e) = rec.stop_recording() {
                    eprintln!("Error stopping recording: {}", e);
                } else {
                    // Recording stopped successfully, generate thumbnail
                    let path_opt = last_path.lock().unwrap().clone();
                    if let Some(video_path) = path_opt {
                        let app_weak_thumb = app_weak.clone();
                        // Run thumbnail generation in background
                        std::thread::spawn(move || {
                            let thumb_path = "/tmp/roton_thumb.jpg";
                            let _ = std::process::Command::new("ffmpeg")
                                .args(&["-y", "-i", &video_path, "-ss", "00:00:01", "-vframes", "1", thumb_path])
                                .output();
                            
                            // Load image inside the event loop because slint::Image is not Send
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Ok(img) = slint::Image::load_from_path(std::path::Path::new(thumb_path)) {
                                    if let Some(app) = app_weak_thumb.upgrade() {
                                        app.set_last_thumbnail(img);
                                    }
                                }
                            });
                        });
                    }
                }
            }
        }
    });

    app.on_select_area({
        let app_weak = app.as_weak();
        move || {
            if let Some(app) = app_weak.upgrade() {
                // Hide app for slurp
                app.hide().unwrap();
                
                // Run slurp
                let output = std::process::Command::new("slurp")
                    .output();
                
                if let Ok(out) = output {
                    if out.status.success() {
                        let geo = String::from_utf8_lossy(&out.stdout).trim().to_string();
                        app.set_recording_mode("selection".into());
                        app.set_recording_geometry(geo.into());
                    }
                }
                
                // Show app again and go home
                app.show().unwrap();
                app.set_active_page(0);
            }
        }
    });

    app.on_open_folder({
        let app_weak = app.as_weak();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let path = app.get_save_path().to_string();
                let _ = std::process::Command::new("xdg-open")
                    .arg(path)
                    .spawn();
            }
        }
    });

    app.run()?;

    Ok(())
}
