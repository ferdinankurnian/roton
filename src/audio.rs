use std::process::Command;

#[derive(Debug, Clone)]
pub struct AudioDevice {
    pub name: String,        // Internal PulseAudio name (e.g., alsa_input.pci-...)
    pub description: String, // Friendly name for UI (e.g., Built-in Audio Analog Stereo)
    pub is_monitor: bool,    // True if it's a monitor of a sink (output loopback)
}

pub fn get_audio_devices() -> Vec<AudioDevice> {
    let output = Command::new("pactl")
        .arg("list")
        .arg("sources")
        .output();

    let mut devices = Vec::new();

    if let Ok(out) = output {
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            
            // Simple state machine parser for pactl output
            let mut current_name = String::new();
            let mut current_desc = String::new();
            let mut current_monitor_of = String::new();
            
            for line in stdout.lines() {
                let trimmed = line.trim();
                
                if line.starts_with("Source #") {
                    // Save previous device if valid
                    if !current_name.is_empty() {
                        devices.push(AudioDevice {
                            name: current_name.clone(),
                            description: if current_desc.is_empty() { current_name.clone() } else { current_desc.clone() },
                            is_monitor: current_monitor_of != "n/a" && !current_monitor_of.is_empty(),
                        });
                    }
                    
                    // Reset for new device
                    current_name.clear();
                    current_desc.clear();
                    current_monitor_of.clear();
                } else if trimmed.starts_with("Name:") {
                    current_name = trimmed.trim_start_matches("Name: ").to_string();
                } else if trimmed.starts_with("Description:") {
                    current_desc = trimmed.trim_start_matches("Description: ").to_string();
                } else if trimmed.starts_with("Monitor of Sink:") {
                    current_monitor_of = trimmed.trim_start_matches("Monitor of Sink: ").to_string();
                }
            }
            
            // Push the last device
            if !current_name.is_empty() {
                 devices.push(AudioDevice {
                    name: current_name,
                    description: if current_desc.is_empty() { "Unknown Device".to_string() } else { current_desc },
                    is_monitor: current_monitor_of != "n/a" && !current_monitor_of.is_empty(),
                });
            }
        }
    }

    devices
}
