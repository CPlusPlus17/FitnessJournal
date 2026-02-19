use reqwest::Client;
use serde_json::Value;
use anyhow::{Result, anyhow};

pub struct GrafanaClient {
    client: Client,
    base_url: String,
    api_key: String,
}

impl GrafanaClient {
    pub fn new(base_url: String, api_key: String) -> Self {
        GrafanaClient {
            client: Client::new(),
            base_url,
            api_key,
        }
    }

    pub async fn test_connection(&self) -> Result<()> {
        let url = format!("{}/api/health", self.base_url);
        let response = self.client.get(&url)
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow!("Failed to connect to Grafana: {}", response.status()))
        }
    }

    pub async fn query_influxdb(&self, query: &str) -> Result<Value> {
        // We know the UID is "garmin_influxdb" and ID is 1 from the previous step.
        // The UID-based proxy endpoint failed (404), so we'll try the classic ID-based one.
        // ID 1 was confirmed in the previous curl output.
        let url = format!("{}/api/datasources/proxy/1/query", self.base_url);
        
        let params = [
            ("db", "GarminStats"),
            ("q", query),
            ("epoch", "ms"), // Get time as unix epoch in ms
        ];

        let response = self.client.get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Accept", "application/json")
            .query(&params)
            .send()
            .await?;

        if response.status().is_success() {
            let json = response.json::<Value>().await?;
            Ok(json)
        } else {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            Err(anyhow!("Grafana proxy query failed: {} - {}", status, text))
        }
    }

    pub async fn get_recent_activities(&self, days: i64) -> Result<Vec<crate::models::ActivitySummary>> {
        // Correct field names based on schema discovery
        let query = format!(
            "SELECT \"distance\", \"movingDuration\", \"calories\", \"averageHR\", \"maxHR\", \"activityType\", \"activityName\" FROM \"ActivitySummary\" WHERE time > now() - {}d ORDER BY time DESC",
            days
        );
        
        let json = self.query_influxdb(&query).await?;
        
        // Basic parsing logic (this assumes the order of columns matches the query)
        // In a production app, we should map columns by name dynamically.
        let mut activities = Vec::new();
        
        if let Some(series) = json.get("results").and_then(|r| r.get(0)).and_then(|r| r.get("series")) {
            if let Some(values) = series.as_array().and_then(|s| s.get(0)).and_then(|s| s.get("values")).and_then(|v| v.as_array()) {
                 for row in values {
                    // Manual mapping based on SELECT order:
                    // Index 0: time (always first)
                    // Index 1: distance
                    // Index 2: movingDuration
                    // Index 3: calories
                    // Index 4: averageHR
                    // Index 5: maxHR
                    // Index 6: activityType
                    // Index 7: activityName
                    
                    // Helper to safely get f64
                    let as_f64 = |val: &serde_json::Value| val.as_f64().unwrap_or(0.0);
                    let as_string = |val: &serde_json::Value| val.as_str().unwrap_or("").to_string();
                    let as_i64 = |val: &serde_json::Value| val.as_i64().unwrap_or(0);

                    // usage: row[0] is time (ms epoch)
                    let time_ms = as_i64(&row[0]);
                    let time = chrono::DateTime::<chrono::Utc>::from_utc(
                        chrono::DateTime::from_timestamp_millis(time_ms).unwrap().naive_utc(),
                        chrono::Utc
                    );

                    let activity = crate::models::ActivitySummary {
                        time,
                        name: as_string(&row[7]), 
                        distance_km: as_f64(&row[1]) / 1000.0,
                        duration_minutes: as_f64(&row[2]) / 60.0,
                        calories: as_f64(&row[3]),
                        avg_hr: as_f64(&row[4]),
                        max_hr: as_f64(&row[5]),
                        sport: as_string(&row[6]),
                        sub_sport: "".to_string(),
                    };

                    if activity.duration_minutes > 0.1 && activity.name != "END" {
                        activities.push(activity);
                    }
                 }
            }
        }
        
        Ok(activities)
    }
}
