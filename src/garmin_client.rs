use crate::garmin_api::GarminApi;
use crate::models::GarminResponse;
use anyhow::{Context, Result};
use tracing::{error, info};

use crate::db::Database;
use std::sync::Arc;
use tokio::sync::Mutex;

pub const AI_WORKOUT_PREFIX: &str = "FJ-AI:";

pub fn is_ai_managed_workout(name: &str) -> bool {
    name.starts_with(AI_WORKOUT_PREFIX)
}

pub fn ensure_ai_workout_name(name: &str) -> String {
    if is_ai_managed_workout(name) {
        name.to_string()
    } else {
        format!("{AI_WORKOUT_PREFIX}{name}")
    }
}

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

                let cache_ttl = std::env::var("GARMIN_CACHE_TTL_SECONDS")
                    .unwrap_or_else(|_| "300".to_string())
                    .parse::<u64>()
                    .unwrap_or(300);

                if elapsed < cache_ttl {
                    // Check Cache
                    info!("Using cached Garmin data ({} mins old)...", elapsed / 60);
                    let response: GarminResponse = serde_json::from_str(&cached_data)
                        .context("Failed to parse cached Garmin JSON output")?;
                    return Ok(response);
                }
            }
        }

        // 2. Fetch Fresh Data natively via Rust GarminApi
        let activities = match self.api.get_activities(0, 100).await {
            Ok(acts) => acts,
            Err(e) => {
                error!("Failed to fetch activities from Garmin: {}", e);
                Vec::new()
            }
        };

        let plans = self
            .api
            .get_training_plans()
            .await
            .ok()
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
                    if let Some(dn) = v.get("displayName").and_then(|val| val.as_str()) {
                        display_name = dn.to_string();
                    }
                    serde_json::from_value(v).unwrap_or(None)
                }
                Err(e) => {
                    info!("Error fetching user profile: {}", e);
                    None
                }
            };

        let today = chrono::Local::now();
        let today_str = today.format("%Y-%m-%d").to_string();
        let max_metrics = match self.api.get_max_metrics(&today_str).await {
            Ok(v) => serde_json::from_value(v).unwrap_or(None),
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
                            Err(e) => info!(
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
            Err(e) => info!("Error fetching Body Battery: {}", e),
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
            Err(e) => info!("Error fetching Sleep Data: {}", e),
        }

        match self.api.get_training_readiness(&today_str).await {
            Ok(tr_json) => {
                if let Some(arr) = tr_json.as_array() {
                    if let Some(first) = arr.first() {
                        recovery_metrics.training_readiness = first
                            .get("score")
                            .and_then(|v| v.as_i64())
                            .map(|v| v as i32);
                    }
                }
            }
            Err(e) => info!("Error fetching Training Readiness: {}", e),
        }

        match self.api.get_hrv_status(&today_str).await {
            Ok(hrv_json) => {
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
            Err(e) => info!("Error fetching HRV JSON: {}", e),
        }

        let seven_days_ago_str = (today - chrono::Duration::days(7))
            .format("%Y-%m-%d")
            .to_string();

        match self
            .api
            .get_rhr_trend(&display_name, &seven_days_ago_str, &today_str)
            .await
        {
            Ok(rhr_json) => {
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
            Err(e) => info!("Error fetching RHR TREND: {}", e),
        }

        let mut final_activities = Vec::new();
        for mut act in activities {
            let is_strength = act.get_activity_type() == Some("strength_training");

            if is_strength {
                if let Some(id) = act.id {
                    match self.api.get_activity_exercise_sets(id).await {
                        Ok(Some(sets)) => {
                            act.sets = Some(sets);
                        }
                        Ok(None) => {
                            info!("No exercise sets returned for strength activity {}", id);
                        }
                        Err(e) => {
                            error!(
                                "Failed to fetch/parse exercise sets for activity {}: {}",
                                id, e
                            );
                        }
                    }
                }
            }
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
            error!("Warning: Failed to write to Garmin cache in DB: {}", e);
        }

        Ok(response)
    }

    pub async fn cleanup_ai_workouts(&self) -> Result<()> {
        info!("Fetching workouts to delete (future only)...");
        let workouts = self.api.get_workouts().await?;
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();

        if let Some(arr) = workouts.as_array() {
            let mut to_delete = Vec::new();
            for w in arr {
                if let Some(name) = w.get("workoutName").and_then(|n| n.as_str()) {
                    if is_ai_managed_workout(name) {
                        if let Some(wid) = w.get("workoutId").and_then(|i| i.as_i64()) {
                            to_delete.push((wid, name.to_string()));
                        }
                    }
                }
            }

            // Also check scheduled dates from the calendar to only delete future ones
            let calendar_dates = self.get_ai_workout_schedule_dates().await;

            info!("Found {} AI workouts total.", to_delete.len());
            for (wid, name) in to_delete {
                // Only delete if scheduled today or in the future, or if we can't determine the date
                let scheduled_date = calendar_dates.get(&name);
                let is_future = match scheduled_date {
                    Some(date) => date.as_str() >= today.as_str(),
                    None => true, // unknown date = safe to delete (orphaned workout)
                };

                if is_future {
                    let endpoint = format!("/workout-service/workout/{}", wid);
                    match self.api.connectapi_delete(&endpoint).await {
                        Ok(_) => info!("Deleted {} ({})", wid, name),
                        Err(e) => info!("Failed to delete {}: {}", wid, e),
                    }
                } else {
                    info!("Keeping past workout {} ({})", wid, name);
                }
            }
        }
        Ok(())
    }

    /// Helper: build a map of AI workout name -> scheduled date from the Garmin calendar
    async fn get_ai_workout_schedule_dates(&self) -> std::collections::HashMap<String, String> {
        let mut dates = std::collections::HashMap::new();
        let today = chrono::Local::now();
        let mut tz_year = today
            .format("%Y")
            .to_string()
            .parse::<i32>()
            .unwrap_or(2025);
        let mut tz_month = today.format("%m").to_string().parse::<i32>().unwrap_or(1) - 1;

        for _ in 0..2 {
            if let Ok(calendar_json) = self.api.get_calendar(tz_year, tz_month).await {
                if let Some(items) = calendar_json
                    .get("calendarItems")
                    .and_then(|i| i.as_array())
                {
                    for item in items {
                        if let (Some(title), Some(date)) = (
                            item.get("title").and_then(|t| t.as_str()),
                            item.get("date").and_then(|d| d.as_str()),
                        ) {
                            if is_ai_managed_workout(title) {
                                dates.insert(title.to_string(), date.to_string());
                            }
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
        dates
    }

    pub async fn create_and_schedule_workout(
        &self,
        workout_spec: &serde_json::Value,
    ) -> Result<String> {
        let builder = crate::workout_builder::WorkoutBuilder::new();
        let mut payload = builder.build_workout_payload(workout_spec, false);
        let mut workout_id = None;
        let mut msg = String::new();

        match self
            .api
            .connectapi_post("/workout-service/workout", &payload)
            .await
        {
            Ok(res) => {
                if let Some(id) = res.get("workoutId").and_then(|i| i.as_i64()) {
                    workout_id = Some(id);
                    msg.push_str(&format!("Created Workout ID: {}. ", id));
                }
            }
            Err(e) => {
                if e.to_string().contains("400") {
                    payload = builder.build_workout_payload(workout_spec, true);
                    match self
                        .api
                        .connectapi_post("/workout-service/workout", &payload)
                        .await
                    {
                        Ok(res) => {
                            if let Some(id) = res.get("workoutId").and_then(|i| i.as_i64()) {
                                workout_id = Some(id);
                                msg.push_str(&format!("Created (Generic) Workout ID: {}. ", id));
                            }
                        }
                        Err(e2) => {
                            return Err(anyhow::anyhow!("Failed to create generic workout: {}", e2))
                        }
                    }
                } else {
                    return Err(anyhow::anyhow!("Failed to create workout: {}", e));
                }
            }
        }

        if let (Some(id), Some(sch_date)) = (
            workout_id,
            workout_spec.get("scheduledDate").and_then(|d| d.as_str()),
        ) {
            let sched_payload = serde_json::json!({ "date": sch_date });
            let sched_endpoint = format!("/workout-service/schedule/{}", id);
            match self
                .api
                .connectapi_post(&sched_endpoint, &sched_payload)
                .await
            {
                Ok(_) => {
                    msg.push_str(&format!("Successfully scheduled on {}.", sch_date));
                    Ok(msg)
                }
                Err(e) => Err(anyhow::anyhow!("Failed to schedule: {}", e)),
            }
        } else {
            Err(anyhow::anyhow!(
                "Could not schedule: missing workout id or date."
            ))
        }
    }

    /// Validates scheduled FJ-AI strength workouts against `generated_workouts.json`.
    /// Returns a list of human-readable correction messages (empty if everything matches).
    pub async fn validate_and_fix_strength_workouts(&self) -> Result<Vec<String>> {
        let workouts_path = std::env::var("GENERATED_WORKOUTS_PATH")
            .unwrap_or_else(|_| "generated_workouts.json".to_string());

        let json_str = match std::fs::read_to_string(&workouts_path) {
            Ok(s) => s,
            Err(_) => {
                info!("No generated_workouts.json found. Skipping strength validation.");
                return Ok(Vec::new());
            }
        };

        let expected: Vec<serde_json::Value> = match serde_json::from_str(&json_str) {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to parse generated_workouts.json: {}", e);
                return Ok(Vec::new());
            }
        };

        if expected.is_empty() {
            return Ok(Vec::new());
        }

        // Only validate workouts scheduled today or in the future
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let expected_future: Vec<&serde_json::Value> = expected
            .iter()
            .filter(|w| {
                w.get("scheduledDate")
                    .and_then(|d| d.as_str())
                    .map(|d| d >= today.as_str())
                    .unwrap_or(false)
            })
            .collect();

        if expected_future.is_empty() {
            info!("All generated workouts are in the past. Skipping strength validation.");
            return Ok(Vec::new());
        }

        // Fetch all workouts from Garmin
        let garmin_workouts = self.api.get_workouts().await?;
        let garmin_arr = garmin_workouts.as_array().unwrap_or(&Vec::new()).clone();

        // Build a map: workout name -> (workoutId, garmin_value)
        let mut garmin_map: std::collections::HashMap<String, (i64, serde_json::Value)> =
            std::collections::HashMap::new();
        for gw in &garmin_arr {
            if let (Some(name), Some(id)) = (
                gw.get("workoutName").and_then(|n| n.as_str()),
                gw.get("workoutId").and_then(|i| i.as_i64()),
            ) {
                if is_ai_managed_workout(name) {
                    garmin_map.insert(name.to_string(), (id, gw.clone()));
                }
            }
        }

        let mut corrections = Vec::new();

        for expected_workout in expected_future {
            let raw_name = expected_workout
                .get("workoutName")
                .and_then(|n| n.as_str())
                .unwrap_or("Unknown");
            let workout_name = ensure_ai_workout_name(raw_name);
            let scheduled_date = expected_workout
                .get("scheduledDate")
                .and_then(|d| d.as_str())
                .unwrap_or("Unknown");

            match garmin_map.get(&workout_name) {
                None => {
                    // Workout is missing from Garmin → re-create
                    info!(
                        "Strength validation: '{}' missing from Garmin. Re-creating...",
                        workout_name
                    );

                    let mut spec = expected_workout.clone();
                    if let Some(obj) = spec.as_object_mut() {
                        obj.insert(
                            "workoutName".to_string(),
                            serde_json::Value::String(workout_name.clone()),
                        );
                    }

                    match self.create_and_schedule_workout(&spec).await {
                        Ok(msg) => {
                            corrections.push(format!(
                                "🔄 Re-created missing workout: {} ({})\n{}",
                                workout_name, scheduled_date, msg
                            ));
                        }
                        Err(e) => {
                            error!("Failed to re-create {}: {}", workout_name, e);
                        }
                    }
                }
                Some((garmin_id, _)) => {
                    // Workout exists – fetch full detail and compare steps
                    let garmin_detail = match self.api.get_workout_by_id(*garmin_id).await {
                        Ok(d) => d,
                        Err(e) => {
                            error!(
                                "Failed to fetch detail for workout {} ({}): {}",
                                garmin_id, workout_name, e
                            );
                            continue;
                        }
                    };

                    if !Self::workout_steps_match(expected_workout, &garmin_detail) {
                        info!(
                            "Strength validation: '{}' has drifted. Deleting and re-creating...",
                            workout_name
                        );

                        // Delete old
                        let endpoint = format!("/workout-service/workout/{}", garmin_id);
                        if let Err(e) = self.api.connectapi_delete(&endpoint).await {
                            error!("Failed to delete old workout {}: {}", garmin_id, e);
                            continue;
                        }

                        // Re-create
                        let mut spec = expected_workout.clone();
                        if let Some(obj) = spec.as_object_mut() {
                            obj.insert(
                                "workoutName".to_string(),
                                serde_json::Value::String(workout_name.clone()),
                            );
                        }

                        match self.create_and_schedule_workout(&spec).await {
                            Ok(msg) => {
                                corrections.push(format!(
                                    "🔄 Fixed drifted workout: {} ({})\n{}",
                                    workout_name, scheduled_date, msg
                                ));
                            }
                            Err(e) => {
                                error!("Failed to re-create {}: {}", workout_name, e);
                            }
                        }
                    } else {
                        info!("Strength validation: '{}' is in sync ✓", workout_name);
                    }
                }
            }
        }

        Ok(corrections)
    }

    /// Compare the expected AI workout steps vs what Garmin currently has.
    /// Returns true if they are equivalent.
    fn workout_steps_match(expected: &serde_json::Value, garmin: &serde_json::Value) -> bool {
        let expected_steps = expected.get("steps").and_then(|s| s.as_array());
        let garmin_segments = garmin.get("workoutSegments").and_then(|s| s.as_array());

        // Count active exercise steps (interval phase) in expected
        let expected_intervals: Vec<&serde_json::Value> = match expected_steps {
            Some(steps) => steps
                .iter()
                .filter(|s| {
                    s.get("phase")
                        .and_then(|p| p.as_str())
                        .map(|p| p == "interval")
                        .unwrap_or(false)
                })
                .collect(),
            None => return true, // no steps defined = nothing to validate
        };

        // Count active exercise steps in Garmin workout segments
        let mut garmin_exercise_count = 0;
        if let Some(segments) = garmin_segments {
            for seg in segments {
                if let Some(steps) = seg.get("workoutSteps").and_then(|s| s.as_array()) {
                    for step in steps {
                        let step_type = step
                            .get("stepType")
                            .and_then(|t| t.get("stepTypeKey"))
                            .and_then(|k| k.as_str())
                            .unwrap_or("");
                        if step_type == "exercise" || step_type == "interval" {
                            garmin_exercise_count += 1;
                        }
                    }
                }
            }
        }

        if expected_intervals.len() != garmin_exercise_count {
            info!(
                "Step count mismatch: expected {} interval steps, Garmin has {} exercise steps",
                expected_intervals.len(),
                garmin_exercise_count
            );
            return false;
        }

        true
    }
}
