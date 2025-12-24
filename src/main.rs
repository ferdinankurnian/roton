
use std::error::Error;
use std::sync::{Arc, Mutex};

mod recorder;
mod config;
mod audio;

use recorder::Recorder;
use config::Settings;
use audio::AudioDevice;
use slint::Model;

slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {

    let app = AppWindow::new()?;
    let last_path = Arc::new(Mutex::new(None));
    
    // Store audio devices to map friendly names back to internal names
    let audio_devices = Arc::new(Mutex::new(Vec::<AudioDevice>::new()));

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

    // Check dependencies
    let has_slurp = Recorder::is_installed("slurp");
    let has_ffmpeg = Recorder::is_installed("ffmpeg");
    app.set_has_slurp(has_slurp);
    app.set_has_ffmpeg(has_ffmpeg);
    
    // Refresh audio devices logic
    let refresh_audio = {
        let app_weak = app.as_weak();
        let audio_devices = audio_devices.clone();
        move || {
            let devices = audio::get_audio_devices();
            let mut monitors = Vec::new();
            let mut mics = Vec::new();
            
            // Populate lists
            for dev in &devices {
                if dev.is_monitor {
                    monitors.push(slint::SharedString::from(&dev.description));
                } else {
                    mics.push(slint::SharedString::from(&dev.description));
                }
            }
            
            // Update UI
            if let Some(app) = app_weak.upgrade() {
                let monitors_model = std::rc::Rc::new(slint::VecModel::from(monitors));
                let mics_model = std::rc::Rc::new(slint::VecModel::from(mics));
                app.set_available_monitors(monitors_model.clone().into());
                app.set_available_mics(mics_model.clone().into());
                
                // Select first if not set (optional logic, Slint might handle empty selection)
                if app.get_selected_monitor() == "" && monitors_model.row_count() > 0 {
                    app.set_selected_monitor(monitors_model.row_data(0).unwrap());
                }
                if app.get_selected_mic() == "" && mics_model.row_count() > 0 {
                    app.set_selected_mic(mics_model.row_data(0).unwrap());
                }
            }
            
            // Store for lookup
            if let Ok(mut store) = audio_devices.lock() {
                *store = devices;
            }
        }
    };
    
    // Initial refresh
    refresh_audio();
    
    app.on_refresh_devices(refresh_audio.clone());

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
        let audio_devices = audio_devices.clone();
        
        move |mode, geometry| {
            let app = app_weak.upgrade().unwrap();
            let save_dir = app.get_save_path().to_string();
            let audio_mode = app.get_audio_mode().to_string();
            
            // Get selected devices
            let selected_monitor = app.get_selected_monitor().to_string();
            let selected_mic = app.get_selected_mic().to_string();
            
            // Resolve to internal names
            let mut mic_arg = None;
            let mut monitor_arg = None;
            
            if let Ok(devices) = audio_devices.lock() {
                 if let Some(dev) = devices.iter().find(|d| d.description == selected_mic) {
                     mic_arg = Some(dev.name.clone());
                 }
                 if let Some(dev) = devices.iter().find(|d| d.description == selected_monitor) {
                     monitor_arg = Some(dev.name.clone());
                 }
            }

            println!("Starting recording: mode={}, geometry={}, path={}, audio={}, mic={:?}, monitor={:?}", 
                mode, geometry, save_dir, audio_mode, mic_arg, monitor_arg);
            
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

                if let Err(e) = rec.start_recording(&path_str, geo, &audio_mode, mic_arg.as_deref(), monitor_arg.as_deref()) {
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
