use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use directories::ProjectDirs;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Settings {
    pub save_path: String,
    pub audio_mode: String,
}

impl Default for Settings {
    fn default() -> Self {
        let mut save_path = String::new();
        if let Some(user_dirs) = directories::UserDirs::new() {
            if let Some(video_dir) = user_dirs.video_dir() {
                save_path = video_dir.to_string_lossy().to_string();
            } else {
                save_path = user_dirs.home_dir().to_string_lossy().to_string();
            }
        }

        Self {
            save_path,
            audio_mode: "Mute".to_string(), // Matches Slint UI default
        }
    }
}

impl Settings {
    fn get_config_path() -> Option<PathBuf> {
        ProjectDirs::from("com", "ferdinankurnian", "roton")
            .map(|proj_dirs| proj_dirs.config_dir().join("config.json"))
    }

    pub fn load() -> Self {
        if let Some(path) = Self::get_config_path() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(mut settings) = serde_json::from_str::<Self>(&content) {
                    // Validate audio_mode
                    let valid_modes = ["Mute", "Screen", "Mic", "Both"];
                    if !valid_modes.contains(&settings.audio_mode.as_str()) {
                        settings.audio_mode = "Mute".to_string();
                    }
                    return settings;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(path) = Self::get_config_path() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let content = serde_json::to_string_pretty(self)?;
            fs::write(path, content)?;
        }
        Ok(())
    }
}
