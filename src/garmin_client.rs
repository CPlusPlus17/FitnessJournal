use std::process::Command;
use anyhow::{Result, Context};
use crate::models::{GarminResponse};

pub struct GarminClient;

impl GarminClient {
    pub fn new() -> Self {
        Self
    }

    pub fn fetch_data(&self) -> Result<GarminResponse> {
        // Run the python script using the venv
        let output = Command::new(".venv/bin/python3")
            .arg("scripts/garmin_fetch.py")
            .output()
            .context("Failed to execute python fetch script")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Python script execution failed: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let response: GarminResponse = serde_json::from_str(&stdout)
            .context("Failed to parse Garmin JSON output")?;

        Ok(response)
    }
}
