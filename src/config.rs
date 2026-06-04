use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Workspace {
    pub name: String,
    pub save_path: String,
    #[serde(default = "default_format")]
    pub selected_format: String,
    #[serde(default)]
    pub selected_monitor: Option<String>,
    #[serde(default)]
    pub selected_mic: Option<String>,
    #[serde(default = "default_audio_mode")]
    pub mic_mode: String,
    #[serde(default = "default_screen_mode")]
    pub screen_mode: String,
    #[serde(default)]
    pub selected_area: Option<String>,
    #[serde(default = "default_show_cursor")]
    pub show_cursor: bool,
    #[serde(default)]
    pub record_screen_sound: bool,
    #[serde(default = "default_theme")]
    pub theme: String,
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            name: "Default".to_string(),
            save_path: default_save_path(),
            selected_format: default_format(),
            selected_monitor: None,
            selected_mic: None,
            mic_mode: default_audio_mode(),
            screen_mode: default_screen_mode(),
            selected_area: None,
            show_cursor: default_show_cursor(),
            record_screen_sound: false,
            theme: default_theme(),
        }
    }
}

impl Workspace {
    pub fn named(name: &str) -> Self {
        let name = name.trim();
        Self {
            name: name.to_string(),
            save_path: PathBuf::from(default_save_path())
                .join(name)
                .to_string_lossy()
                .to_string(),
            ..Self::default()
        }
    }

    fn from_legacy(save_path: Option<String>, audio_mode: Option<String>) -> Self {
        let mut workspace = Self {
            save_path: save_path.unwrap_or_else(default_save_path),
            ..Self::default()
        };

        match audio_mode.as_deref() {
            Some("Mic") => workspace.mic_mode = "Mic".to_string(),
            Some("Screen") => workspace.record_screen_sound = true,
            Some("Both") => {
                workspace.mic_mode = "Mic".to_string();
                workspace.record_screen_sound = true;
            }
            _ => {}
        }

        workspace
    }

    fn validate(&mut self) {
        if self.name.trim().is_empty() {
            self.name = "Workspace".to_string();
        }

        if self.save_path.trim().is_empty() {
            self.save_path = default_save_path();
        }

        if !["MP4", "MKV", "WEBM"].contains(&self.selected_format.as_str()) {
            self.selected_format = default_format();
        }

        if !["Mute", "Mic"].contains(&self.mic_mode.as_str()) {
            self.mic_mode = default_audio_mode();
        }

        if !["Fullscreen", "SelectArea"].contains(&self.screen_mode.as_str()) {
            self.screen_mode = default_screen_mode();
        }

        if !["Blue", "Green", "Red", "Purple", "Amber"].contains(&self.theme.as_str()) {
            self.theme = default_theme();
        }
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct Settings {
    pub minimize_to_tray: bool,
    pub active_workspace: usize,
    pub workspaces: Vec<Workspace>,
}

#[derive(Deserialize, Default)]
struct StoredSettings {
    #[serde(default)]
    minimize_to_tray: bool,
    #[serde(default)]
    active_workspace: usize,
    #[serde(default)]
    workspaces: Vec<Workspace>,
    #[serde(default)]
    save_path: Option<String>,
    #[serde(default)]
    audio_mode: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            minimize_to_tray: false,
            active_workspace: 0,
            workspaces: vec![Workspace::default()],
        }
    }
}

impl Settings {
    fn get_config_path() -> Option<PathBuf> {
        ProjectDirs::from("com", "ferdinankurnian", "roton")
            .map(|proj_dirs| proj_dirs.config_dir().join("config.json"))
    }

    pub fn load() -> Self {
        Self::get_config_path()
            .and_then(|path| fs::read_to_string(path).ok())
            .and_then(|content| Self::from_json(&content))
            .unwrap_or_default()
    }

    fn from_json(content: &str) -> Option<Self> {
        let stored = serde_json::from_str::<StoredSettings>(content).ok()?;
        let mut workspaces = stored.workspaces;

        if workspaces.is_empty() {
            workspaces.push(Workspace::from_legacy(stored.save_path, stored.audio_mode));
        }

        for workspace in &mut workspaces {
            workspace.validate();
        }

        Some(Self {
            minimize_to_tray: stored.minimize_to_tray,
            active_workspace: stored.active_workspace.min(workspaces.len() - 1),
            workspaces,
        })
    }

    pub fn active_workspace(&self) -> &Workspace {
        &self.workspaces[self.active_workspace]
    }

    pub fn active_workspace_mut(&mut self) -> &mut Workspace {
        &mut self.workspaces[self.active_workspace]
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

fn default_save_path() -> String {
    directories::UserDirs::new()
        .map(|user_dirs| {
            user_dirs
                .video_dir()
                .unwrap_or_else(|| user_dirs.home_dir())
                .to_string_lossy()
                .to_string()
        })
        .unwrap_or_default()
}

fn default_format() -> String {
    "MP4".to_string()
}

fn default_audio_mode() -> String {
    "Mute".to_string()
}

fn default_screen_mode() -> String {
    "Fullscreen".to_string()
}

fn default_show_cursor() -> bool {
    true
}

fn default_theme() -> String {
    "Blue".to_string()
}

#[cfg(test)]
mod tests {
    use super::{Settings, Workspace};

    #[test]
    fn migrates_legacy_audio_mode_into_default_workspace() {
        let settings = Settings::from_json(
            r#"{
                "save_path": "/tmp/videos",
                "audio_mode": "Both",
                "minimize_to_tray": true
            }"#,
        )
        .unwrap();

        let workspace = settings.active_workspace();
        assert_eq!(workspace.name, "Default");
        assert_eq!(workspace.save_path, "/tmp/videos");
        assert_eq!(workspace.mic_mode, "Mic");
        assert!(workspace.record_screen_sound);
        assert!(settings.minimize_to_tray);
    }

    #[test]
    fn clamps_invalid_active_workspace() {
        let settings = Settings::from_json(
            r#"{
                "active_workspace": 99,
                "workspaces": [
                    { "name": "Default", "save_path": "/tmp/videos" }
                ]
            }"#,
        )
        .unwrap();

        assert_eq!(settings.active_workspace, 0);
    }

    #[test]
    fn preserves_workspace_node_state() {
        let mut workspace = Workspace::named("Editing");
        workspace.selected_format = "WEBM".to_string();
        workspace.selected_monitor = Some("DP-1".to_string());
        workspace.selected_mic = Some("Studio Mic".to_string());
        workspace.mic_mode = "Mic".to_string();
        workspace.screen_mode = "SelectArea".to_string();
        workspace.selected_area = Some("10,20 1280x720".to_string());
        workspace.show_cursor = false;
        workspace.record_screen_sound = true;
        workspace.theme = "Green".to_string();

        let content = serde_json::to_string(&Settings {
            minimize_to_tray: false,
            active_workspace: 0,
            workspaces: vec![workspace],
        })
        .unwrap();
        let settings = Settings::from_json(&content).unwrap();
        let workspace = settings.active_workspace();

        assert_eq!(workspace.selected_format, "WEBM");
        assert_eq!(workspace.selected_monitor.as_deref(), Some("DP-1"));
        assert_eq!(workspace.selected_mic.as_deref(), Some("Studio Mic"));
        assert_eq!(workspace.mic_mode, "Mic");
        assert_eq!(workspace.screen_mode, "SelectArea");
        assert_eq!(workspace.selected_area.as_deref(), Some("10,20 1280x720"));
        assert!(!workspace.show_cursor);
        assert!(workspace.record_screen_sound);
        assert_eq!(workspace.theme, "Green");
    }

    #[test]
    fn resets_invalid_workspace_theme() {
        let settings = Settings::from_json(
            r#"{
                "workspaces": [
                    {
                        "name": "Default",
                        "save_path": "/tmp/videos",
                        "theme": "Hotdog"
                    }
                ]
            }"#,
        )
        .unwrap();

        assert_eq!(settings.active_workspace().theme, "Blue");
    }
}
