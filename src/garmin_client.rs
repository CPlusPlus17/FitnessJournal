use crate::garmin_api::GarminApi;
use crate::models::GarminResponse;
use anyhow::{Context, Result};

use crate::db::Database;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct GarminClient {
    pub api: GarminApi,
    pub db: Arc<Mutex<Database>>,
}

impl GarminClient {
    pub fn new(db: Arc<Mutex<Database>>) -> Self {
        Self {
            api: GarminApi::new().expect("Failed to initialize GarminApi"),
            db,
        }
    }

    pub async fn fetch_data(&self) -> Result<GarminResponse> {
        // 1. Check Cache
        let is_test = std::env::args().any(|a| a == "--test");
        if !is_test {
            if let Ok(Some((cached_data, updated_at))) = self.db.lock().await.get_garmin_cache() {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let elapsed = now.saturating_sub(updated_at);

                if elapsed < 3600 {
                    // 1 Hour
                    println!("Using cached Garmin data ({} mins old)...", elapsed / 60);
                    let response: GarminResponse = serde_json::from_str(&cached_data)
                        .context("Failed to parse cached Garmin JSON output")?;
                    return Ok(response);
                }
            }
        }

        // 2. Fetch Fresh Data natively via Rust GarminApi
        let activities = match self.api.get_activities(0, 50).await {
            Ok(acts) => acts,
            Err(e) => {
                eprintln!("Failed to fetch activities from Garmin: {}", e);
                Vec::new()
            }
        };

        let plans = match self.api.get_training_plans().await {
            Ok(p) => Some(p),
            Err(_) => None,
        }
        .unwrap_or(serde_json::Value::Null); // we will wrap loosely
        let plans_vec = if plans.is_array() {
            serde_json::from_value(plans).unwrap_or_default()
        } else {
            Vec::new()
        };

        let mut display_name = String::new();
        let user_profile: Option<crate::models::GarminProfile> =
            match self.api.get_user_profile().await {
                Ok(v) => {
                    println!("RAW USER PROFILE: {}", v);
                    if let Some(dn) = v.get("displayName").and_then(|val| val.as_str()) {
                        display_name = dn.to_string();
                    }
                    serde_json::from_value(v).unwrap_or(None)
                }
                Err(e) => {
                    println!("Error fetching user profile: {}", e);
                    None
                }
            };

        let today = chrono::Local::now();
        let today_str = today.format("%Y-%m-%d").to_string();
        let max_metrics = match self.api.get_max_metrics(&today_str).await {
            Ok(v) => {
                println!("RAW MAX METRICS: {}", v);
                serde_json::from_value(v).unwrap_or(None)
            }
            Err(_) => None,
        };

        // Fetch Calendar for Scheduled Workouts
        let mut scheduled_workouts = Vec::new();
        let mut seen_keys = std::collections::HashSet::new();
        let mut tz_year = today
            .format("%Y")
            .to_string()
            .parse::<i32>()
            .unwrap_or(2025);
        let mut tz_month = today.format("%m").to_string().parse::<i32>().unwrap_or(1) - 1;

        for _ in 0..6 {
            if let Ok(calendar_json) = self.api.get_calendar(tz_year, tz_month).await {
                if let Some(items) = calendar_json
                    .get("calendarItems")
                    .and_then(|i| i.as_array())
                {
                    for item in items {
                        // Item type can be "workout" or "activity" maybe?
                        match serde_json::from_value::<crate::models::ScheduledWorkout>(
                            item.clone(),
                        ) {
                            Ok(sw) => {
                                if let Some(ref it) = sw.item_type {
                                    if it == "workout"
                                        || it == "fbtAdaptiveWorkout"
                                        || it == "race"
                                        || it == "event"
                                        || it == "primaryEvent"
                                    {
                                        let key = format!(
                                            "{}_{}",
                                            sw.date,
                                            sw.title.as_deref().unwrap_or("")
                                        );
                                        if seen_keys.insert(key) {
                                            scheduled_workouts.push(sw);
                                        }
                                    }
                                }
                            }
                            Err(e) => println!(
                                "Failed to parse calendar item (type: {:?}): {}. Raw: {:?}",
                                item.get("itemType"),
                                e,
                                item
                            ),
                        }
                    }
                }
            }

            tz_month += 1;
            if tz_month > 11 {
                tz_month = 0;
                tz_year += 1;
            }
        }

        // Fetch Recovery Metrics
        let mut recovery_metrics = crate::models::GarminRecoveryMetrics {
            sleep_score: None,
            recent_sleep_scores: Vec::new(),
            current_body_battery: None,
            training_readiness: None,
            hrv_status: None,
            hrv_last_night_avg: None,
            hrv_weekly_avg: None,
            rhr_trend: Vec::new(),
        };

        match self.api.get_body_battery(&today_str).await {
            Ok(bb_json) => {
                if let Some(arr) = bb_json.as_array() {
                    if let Some(latest_day) = arr.last() {
                        if let Some(bb_values) = latest_day
                            .get("bodyBatteryValuesArray")
                            .and_then(|v| v.as_array())
                        {
                            if let Some(latest_tuple) = bb_values.last().and_then(|v| v.as_array())
                            {
                                if latest_tuple.len() >= 2 {
                                    recovery_metrics.current_body_battery =
                                        latest_tuple[1].as_i64().map(|v| v as i32);
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => println!("Error fetching Body Battery: {}", e),
        }

        match self.api.get_sleep_data(&display_name, &today_str).await {
            Ok(sleep_json) => {
                recovery_metrics.sleep_score = sleep_json
                    .get("dailySleepDTO")
                    .and_then(|d| d.get("sleepScores"))
                    .and_then(|s| s.get("overall"))
                    .and_then(|o| o.get("value"))
                    .and_then(|v| v.as_i64())
                    .map(|v| v as i32);
            }
            Err(e) => println!("Error fetching Sleep Data: {}", e),
        }

        match self.api.get_training_readiness(&today_str).await {
            Ok(tr_json) => {
                println!("RAW TRAINING READINESS JSON: {}", tr_json);
                if let Some(arr) = tr_json.as_array() {
                    if let Some(first) = arr.first() {
                        recovery_metrics.training_readiness = first
                            .get("score")
                            .and_then(|v| v.as_i64())
                            .map(|v| v as i32);
                    }
                }
            }
            Err(e) => println!("Error fetching Training Readiness: {}", e),
        }

        match self.api.get_hrv_status(&today_str).await {
            Ok(hrv_json) => {
                println!("RAW HRV JSON: {}", hrv_json);
                if let Some(summary) = hrv_json.get("hrvSummary") {
                    recovery_metrics.hrv_status = summary
                        .get("status")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    recovery_metrics.hrv_weekly_avg = summary
                        .get("weeklyAvg")
                        .and_then(|v| v.as_i64())
                        .map(|v| v as i32);
                    recovery_metrics.hrv_last_night_avg = summary
                        .get("lastNightAvg")
                        .and_then(|v| v.as_i64())
                        .map(|v| v as i32);
                }
            }
            Err(e) => println!("Error fetching HRV JSON: {}", e),
        }

        let seven_days_ago_str = (today - chrono::Duration::try_days(7).unwrap())
            .format("%Y-%m-%d")
            .to_string();

        match self
            .api
            .get_rhr_trend(&display_name, &seven_days_ago_str, &today_str)
            .await
        {
            Ok(rhr_json) => {
                println!("RAW RHR TREND: {}", rhr_json);
                if let Some(arr) = rhr_json.as_array() {
                    let mut trend = Vec::new();
                    for item in arr {
                        // The actual field name will be discovered in debug print, but "value" or "values" is common.
                        // For rhr, it's often { "value": 50 }
                        if let Some(val) = item.get("value").and_then(|v| v.as_i64()) {
                            trend.push(val as i32);
                        } else if let Some(val) = item
                            .get("values")
                            .and_then(|v| v.get("restingHR"))
                            .and_then(|r| r.as_i64())
                        {
                            trend.push(val as i32);
                        }
                    }
                    recovery_metrics.rhr_trend = trend;
                } else if let Some(all_metrics) = rhr_json
                    .get("allMetrics")
                    .and_then(|m| m.get("metricsMap"))
                    .and_then(|m| m.get("WELLNESS_RESTING_HEART_RATE"))
                    .and_then(|a| a.as_array())
                {
                    let mut trend = Vec::new();
                    for item in all_metrics {
                        if let Some(val) = item.get("value").and_then(|v| v.as_f64()) {
                            trend.push(val as i32);
                        } else if let Some(val) = item.get("value").and_then(|v| v.as_i64()) {
                            trend.push(val as i32);
                        }
                    }
                    recovery_metrics.rhr_trend = trend;
                }
            }
            Err(e) => println!("Error fetching RHR TREND: {}", e),
        }

        let mut final_activities = Vec::new();
        for act in activities {
            final_activities.push(act);
        }

        let response = GarminResponse {
            activities: final_activities,
            plans: plans_vec,
            user_profile,
            max_metrics,
            scheduled_workouts,
            recovery_metrics: Some(recovery_metrics),
        };

        let stdout = serde_json::to_string(&response)?;

        // 3. Save to Cache
        if let Err(e) = self.db.lock().await.set_garmin_cache(&stdout) {
            eprintln!("Warning: Failed to write to Garmin cache in DB: {}", e);
        }

        Ok(response)
    }

    pub async fn cleanup_ai_workouts(&self) -> Result<()> {
        println!("Fetching workouts to delete...");
        let workouts = self.api.get_workouts().await?;
        if let Some(arr) = workouts.as_array() {
            let mut to_delete = Vec::new();
            for w in arr {
                if let Some(name) = w.get("workoutName").and_then(|n| n.as_str()) {
                    if name.starts_with("Strength")
                        || name.starts_with("Test")
                        || name.starts_with("Full Body")
                        || name.starts_with("Upper")
                        || name.starts_with("Lower")
                    {
                        if let Some(wid) = w.get("workoutId").and_then(|i| i.as_i64()) {
                            to_delete.push((wid, name.to_string()));
                        }
                    }
                }
            }

            println!("Found {} workouts to delete.", to_delete.len());
            for (wid, name) in to_delete {
                let endpoint = format!("/workout-service/workout/{}", wid);
                match self.api.connectapi_delete(&endpoint).await {
                    Ok(_) => println!("Deleted {} ({})", wid, name),
                    Err(e) => println!("Failed to delete {}: {}", wid, e),
                }
            }
        }
        Ok(())
    }
}
