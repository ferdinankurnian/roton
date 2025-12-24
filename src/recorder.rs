use std::process::{Child, Command, Stdio};
use std::path::PathBuf;
use std::fs;

#[derive(Clone)]
struct RecordingConfig {
    geometry: Option<String>,
    audio_mode: String,
    mic_device: Option<String>,
    monitor_device: Option<String>,
    final_path: String,
}

pub struct Recorder {
    process: Option<Child>,
    pulse_modules: Vec<String>,
    config: Option<RecordingConfig>,
    temp_segments: Vec<PathBuf>,
    is_paused: bool,
}

impl Recorder {
    pub fn new() -> Self {
        Self { 
            process: None,
            pulse_modules: Vec::new(),
            config: None,
            temp_segments: Vec::new(),
            is_paused: false,
        }
    }

    pub fn is_installed(cmd: &str) -> bool {
        Command::new("which")
            .arg(cmd)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    pub fn is_available() -> bool {
        Self::is_installed("wl-screenrec")
    }

    // PulseAudio Helper Methods
    fn load_pulse_module(&mut self, args: &[&str]) -> Option<String> {
        let output = Command::new("pactl")
            .arg("load-module")
            .args(args)
            .output()
            .ok()?;

        if output.status.success() {
            let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
            self.pulse_modules.push(id.clone());
            Some(id)
        } else {
            None
        }
    }

    fn unload_pulse_modules(&mut self) {
        for id in &self.pulse_modules {
            let _ = Command::new("pactl")
                .arg("unload-module")
                .arg(id)
                .status();
        }
        self.pulse_modules.clear();
    }

    // Internal method to start a single segment recording
    fn start_segment(&mut self) -> Result<(), String> {
        if let Some(config) = &self.config {
            // Generate temp file path in system temp dir
            let timestamp = chrono::Local::now().format("%H-%M-%S-%f");
            let temp_file = std::env::temp_dir().join(format!("roton_seg_{}.mp4", timestamp));
            let temp_path_str = temp_file.to_str().unwrap().to_string();

            let mut cmd = Command::new("wl-screenrec");
            cmd.arg("-f").arg(&temp_path_str);

            if let Some(geo) = &config.geometry {
                cmd.arg("-g").arg(geo);
            }

            match config.audio_mode.as_str() {
                "Screen" => {
                    cmd.arg("--audio");
                    if let Some(dev) = &config.monitor_device {
                        cmd.arg("--audio-device").arg(dev);
                    }
                }
                "Mic" => {
                    cmd.arg("--audio");
                    if let Some(dev) = &config.mic_device {
                        cmd.arg("--audio-device").arg(dev);
                    }
                }
                "Both" => {
                    // Use the ALREADY created virtual mixer if possible, 
                    // or rely on the mixer created at start_session.
                    // Since modules are persistent in `pulse_modules`, we just point to the sink monitor.
                     cmd.arg("--audio");
                     cmd.arg("--audio-device").arg("RotonMixer.monitor");
                }
                _ => {}
            }

            match cmd.spawn() {
                Ok(child) => {
                    println!("Started segment: {:?}", temp_file);
                    self.process = Some(child);
                    self.temp_segments.push(temp_file);
                    Ok(())
                }
                Err(e) => Err(format!("Failed to start segment: {}", e)),
            }
        } else {
            Err("No configuration found".to_string())
        }
    }

    fn stop_current_process(&mut self) {
        if let Some(mut child) = self.process.take() {
            let pid = child.id();
            let _ = Command::new("kill").arg("-s").arg("INT").arg(pid.to_string()).status();
            let _ = child.wait();
        }
    }

    // Public API

    pub fn start_session(&mut self, final_path: &str, geometry: Option<&str>, audio_mode: &str, mic: Option<&str>, monitor: Option<&str>) -> Result<(), String> {
        // Clear previous session state
        self.stop_current_process();
        self.unload_pulse_modules();
        self.temp_segments.clear();
        self.is_paused = false;

        // Setup PulseAudio mixer if needed for "Both"
        if audio_mode == "Both" {
             if let (Some(m), Some(mon)) = (mic, monitor) {
                // Setup Mixer
                self.load_pulse_module(&["module-null-sink", "sink_name=RotonMixer", "sink_properties=device.description=RotonMixer"]);
                self.load_pulse_module(&["module-loopback", "sink=RotonMixer", &format!("source={}", m), "latency_msec=1"]);
                self.load_pulse_module(&["module-loopback", "sink=RotonMixer", &format!("source={}", mon), "latency_msec=1"]);
             }
        }

        // Save Config
        self.config = Some(RecordingConfig {
            geometry: geometry.map(|s| s.to_string()),
            audio_mode: audio_mode.to_string(),
            mic_device: mic.map(|s| s.to_string()),
            monitor_device: monitor.map(|s| s.to_string()),
            final_path: final_path.to_string(),
        });

        // Start first segment
        self.start_segment()
    }

    pub fn pause_session(&mut self) -> Result<(), String> {
        if !self.is_paused {
            self.stop_current_process();
            self.is_paused = true;
            println!("Session paused.");
        }
        Ok(())
    }

    pub fn resume_session(&mut self) -> Result<(), String> {
        if self.is_paused {
            self.start_segment()?;
            self.is_paused = false;
            println!("Session resumed.");
        }
        Ok(())
    }

    pub fn finish_session(&mut self) -> Result<(), String> {
        self.stop_current_process();
        self.unload_pulse_modules();

        if self.temp_segments.is_empty() {
            return Err("No recordings made".to_string());
        }

        let final_path = if let Some(cfg) = &self.config {
            cfg.final_path.clone()
        } else {
            return Err("Config lost".to_string());
        };

        println!("Finishing session. Segments: {}", self.temp_segments.len());

        if self.temp_segments.len() == 1 {
            // Try rename first, fallback to copy if cross-device (tmpfs to disk)
            if let Err(e) = fs::rename(&self.temp_segments[0], &final_path) {
                if e.raw_os_error() == Some(18) { // EXDEV: Invalid cross-device link
                    fs::copy(&self.temp_segments[0], &final_path).map_err(|e| e.to_string())?;
                    fs::remove_file(&self.temp_segments[0]).map_err(|e| e.to_string())?;
                } else {
                    return Err(e.to_string());
                }
            }
        } else {
            // Concat multiple files
            // 1. Create list.txt
            let list_path = std::env::temp_dir().join("roton_concat_list.txt");
            let mut list_content = String::new();
            for path in &self.temp_segments {
                 list_content.push_str(&format!("file '{}'\n", path.to_str().unwrap()));
            }
            fs::write(&list_path, list_content).map_err(|e| e.to_string())?;

            // 2. Run FFMPEG Concat
            println!("Concatenating to: {}", final_path);
            let status = Command::new("ffmpeg")
                .arg("-f").arg("concat")
                .arg("-safe").arg("0")
                .arg("-i").arg(&list_path)
                .arg("-c").arg("copy")
                .arg("-y") // overwrite
                .arg(&final_path)
                .stdout(Stdio::null())
                .stderr(Stdio::null()) // maybe log stderr?
                .status()
                .map_err(|e| e.to_string())?;

            if !status.success() {
                return Err("FFmpeg concat failed".to_string());
            }

            // Cleanup temp list
            let _ = fs::remove_file(list_path);
        }

        // Cleanup temp segments
        for path in &self.temp_segments {
            let _ = fs::remove_file(path);
        }
        
        self.config = None;
        self.temp_segments.clear();

        Ok(())
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        // Safety net: ensure cleanup happens when Recorder is dropped (app closing)
        self.stop_current_process();
        self.unload_pulse_modules();
    }
}