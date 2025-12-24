use std::process::{Child, Command, Stdio};

pub struct Recorder {
    process: Option<Child>,
    pulse_modules: Vec<String>, // Stores IDs of loaded PulseAudio modules
}

impl Recorder {
    pub fn new() -> Self {
        Self { 
            process: None,
            pulse_modules: Vec::new(),
        }
    }

    /// Checks if a command is available in the PATH
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
            eprintln!("Failed to load pulse module: {:?}", args);
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

    pub fn start_recording(&mut self, output_path: &str, geometry: Option<&str>, audio_mode: &str, mic_device: Option<&str>, monitor_device: Option<&str>) -> Result<(), String> {
        if self.process.is_some() {
            return Err("Recording already in progress".to_string());
        }

        let mut cmd = Command::new("wl-screenrec");
        cmd.arg("-f").arg(output_path);

        if let Some(geo) = geometry {
            cmd.arg("-g").arg(geo);
        }

        match audio_mode {
            "Screen" => {
                cmd.arg("--audio");
                if let Some(dev) = monitor_device {
                    cmd.arg("--audio-device").arg(dev);
                }
            }
            "Mic" => {
                cmd.arg("--audio");
                if let Some(dev) = mic_device {
                    cmd.arg("--audio-device").arg(dev);
                }
            }
            "Both" => {
                if let (Some(mic), Some(monitor)) = (mic_device, monitor_device) {
                    println!("Setting up audio mixing for devices: {} + {}", mic, monitor);
                    
                    // 1. Create a Null Sink (Virtual Mixer)
                    // sink_name=RotonMixer sink_properties=device.description=RotonMixer
                    if let Some(_) = self.load_pulse_module(&["module-null-sink", "sink_name=RotonMixer", "sink_properties=device.description=RotonMixer"]) {
                        
                        // 2. Loopback Mic -> RotonMixer
                        self.load_pulse_module(&["module-loopback", "sink=RotonMixer", &format!("source={}", mic), "latency_msec=1"]);
                        
                        // 3. Loopback Monitor -> RotonMixer
                        self.load_pulse_module(&["module-loopback", "sink=RotonMixer", &format!("source={}", monitor), "latency_msec=1"]);
                        
                        // 4. Record the monitor of RotonMixer
                        cmd.arg("--audio");
                        cmd.arg("--audio-device").arg("RotonMixer.monitor");
                    } else {
                        return Err("Failed to setup audio mixing (could not create null sink)".to_string());
                    }
                } else {
                    return Err("Both mode requires both Mic and Monitor devices to be selected".to_string());
                }
            }
            _ => {
                // Mute - don't pass --audio flag
            }
        }

        // Don't clutter roton's stdout/stderr
        // cmd.stdout(Stdio::null()); 
        // cmd.stderr(Stdio::null()); // Maybe keep stderr for debugging? 
        
        match cmd.spawn() {
            Ok(child) => {
                println!("Recording started: {} (geometry: {:?}, audio: {})", output_path, geometry, audio_mode);
                self.process = Some(child);
                Ok(())
            }
            Err(e) => {
                // If spawn fails, cleanup any modules we might have loaded
                self.unload_pulse_modules();
                Err(format!("Failed to start recorder: {}", e))
            },
        }
    }

    pub fn stop_recording(&mut self) -> Result<(), String> {
        // Cleanup modules regardless of how the process stops
        defer_cleanup(self);

        if let Some(mut child) = self.process.take() {
            println!("Stopping recording...");
            
            // wl-screenrec needs SIGINT (Ctrl+C) to finalize the file properly.
            // Using standard KILL might corrupt the MP4.
            // Since we are on Linux, we use the `kill` command to send SIGINT.
            let pid = child.id();
            let _ = Command::new("kill")
                .arg("-s")
                .arg("INT")
                .arg(pid.to_string())
                .status();

            // Wait for the process to exit gracefully
            match child.wait() {
                Ok(status) => {
                    if status.success() {
                        println!("Recording stopped successfully.");
                        Ok(())
                    } else {
                        Err(format!("Recorder exited with status: {}", status))
                    }
                }
                Err(e) => Err(format!("Failed to wait for recorder: {}", e)),
            }
        } else {
            Err("No recording in progress".to_string())
        }
    }
}

// Helper to cleanup modules if called directly or on drop
fn defer_cleanup(recorder: &mut Recorder) {
    recorder.unload_pulse_modules();
}
