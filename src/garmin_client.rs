use std::process::Command;
use anyhow::{Result, Context};
use crate::models::{GarminResponse};

pub struct GarminClient;

impl GarminClient {
    pub fn new() -> Self {
        Self
    }

    pub fn fetch_data(&self) -> Result<GarminResponse> {
        let cache_file = "garmin_cache.json";
        
        // 1. Check Cache
        if let Ok(metadata) = std::fs::metadata(cache_file) {
            if let Ok(modified) = metadata.modified() {
                if let Ok(elapsed) = modified.elapsed() {
                    if elapsed.as_secs() < 3600 { // 1 Hour
                        println!("Using cached Garmin data ({} mins old)...", elapsed.as_secs() / 60);
                        let cached_data = std::fs::read_to_string(cache_file)?;
                        let response: GarminResponse = serde_json::from_str(&cached_data)
                            .context("Failed to parse cached Garmin JSON output")?;
                        return Ok(response);
                    }
                }
            }
        }

        // 2. Fetch Fresh Data (Python Script)
        let output = Command::new(".venv/bin/python3")
            .arg("scripts/garmin_fetch.py")
            .output()
            .context("Failed to execute python fetch script")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Python script execution failed: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        
        // 3. Save to Cache
        if let Err(e) = std::fs::write(cache_file, stdout.as_ref()) {
            eprintln!("Warning: Failed to write cache file: {}", e);
        }

        // 4. Parse
        let response: GarminResponse = serde_json::from_str(&stdout)
            .context("Failed to parse Garmin JSON output")?;

        Ok(response)
    }
}
