use std::process::{Child, Command, Stdio};

pub struct Recorder {
    process: Option<Child>,
}

impl Recorder {
    pub fn new() -> Self {
        Self { process: None }
    }

    /// Checks if `wl-screenrec` is available in the PATH
    pub fn is_available() -> bool {
        Command::new("wl-screenrec")
            .arg("--help")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    pub fn start_recording(&mut self, output_path: &str, geometry: Option<&str>, audio_mode: &str) -> Result<(), String> {
        if self.process.is_some() {
            return Err("Recording already in progress".to_string());
        }

        let mut cmd = Command::new("wl-screenrec");
        cmd.arg("-f").arg(output_path);

        if let Some(geo) = geometry {
            cmd.arg("-g").arg(geo);
        }

        match audio_mode {
            "Screen" | "Mic" | "Both" => {
                cmd.arg("--audio");
                if audio_mode == "Mic" {
                    // This is a simplification; usually you want to specify device for mic
                    // wl-screenrec defaults to default capture device (usually mic)
                }
                // wl-screenrec doesn't natively distinguish between 'screen audio' and 'mic' easily 
                // without specific pulseaudio/pipewire source names.
                // For now, --audio will enable the default capture device.
            }
            _ => {} // Mute
        }

        // Don't clutter roton's stdout/stderr
        // cmd.stdout(Stdio::null()); 
        // cmd.stderr(Stdio::null()); // Maybe keep stderr for debugging? 
        
        match cmd.spawn() {
            Ok(child) => {
                println!("Recording started: {} (geometry: {:?})", output_path, geometry);
                self.process = Some(child);
                Ok(())
            }
            Err(e) => Err(format!("Failed to start recorder: {}", e)),
        }
    }

    pub fn stop_recording(&mut self) -> Result<(), String> {
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
