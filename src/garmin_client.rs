use crate::garmin_api::GarminApi;
use crate::models::GarminResponse;
use anyhow::{Context, Result};
use tracing::{error, info};

use crate::db::Database;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Haversine distance in meters between two lat/lng points.
fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6_371_000.0; // Earth radius in meters
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();
    let a = (d_lat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (d_lon / 2.0).sin().powi(2);
    r * 2.0 * a.sqrt().atan2((1.0 - a).sqrt())
}

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
                            Ok(mut sw) => {
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
                                            if it == "fbtAdaptiveWorkout" {
                                                // Try workoutUuid first, then uuid, then id
                                                let target = sw.raw_fields.get("workoutUuid").and_then(|v| v.as_str()).map(|s| s.to_string())
                                                    .or_else(|| sw.raw_fields.get("uuid").and_then(|v| v.as_str()).map(|s| s.to_string()))
                                                    .or_else(|| sw.raw_fields.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()))
                                                    .or_else(|| sw.raw_fields.get("id").and_then(|v| v.as_u64()).map(|n| n.to_string()));

                                                if let Some(target_id) = target {
                                                    match self.api.get_adaptive_workout_details(&target_id).await {
                                                        Ok(details) => sw.adaptive_details = Some(details),
                                                        Err(e) => info!("Failed to get adaptive details for {}: {}", target_id, e),
                                                    }
                                                }
                                            }

                                            // Fetch full workout detail (with segments/steps) for workouts with a workoutId.
                                            // Check: raw calendar item → adaptive_details top-level → nested workout/adaptiveWorkout objects
                                            let wid_val = sw.raw_fields.get("workoutId")
                                                .or_else(|| sw.adaptive_details.as_ref().and_then(|ad| {
                                                    ad.get("workoutId")
                                                        .or_else(|| ad.get("workout").and_then(|w| w.get("workoutId")))
                                                        .or_else(|| ad.get("adaptiveWorkout").and_then(|w| w.get("workoutId")))
                                                }));
                                            let wid_i64 = wid_val.and_then(|v| v.as_i64()).or_else(|| wid_val.and_then(|v| v.as_u64()).map(|u| u as i64));

                                            if let Some(wid) = wid_i64 {
                                                if wid > 0 {
                                                    info!("Fetching workout detail for '{}' (workoutId={})", sw.title.as_deref().unwrap_or("?"), wid);
                                                    match self.api.get_workout_by_id(wid).await {
                                                        Ok(detail) => sw.workout_detail = Some(detail),
                                                        Err(e) => info!("Failed to get workout detail for {}: {}", wid, e),
                                                    }
                                                }
                                            }


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

    /// Creates a loop course on Garmin Connect for a run workout.
    /// Uses Garmin's round-trip route API for real road/trail routes,
    /// falling back to a synthetic circle if the API fails.
    pub async fn create_course_for_workout(
        &self,
        name: &str,
        distance_m: f64,
        lat: f64,
        lng: f64,
    ) -> Result<serde_json::Value> {
        // Try Garmin's round-trip route API first for real road/trail routes
        let geo_points = match self.api.get_round_trip_route(lat, lng, distance_m).await {
            Ok(route_coords) => {
                info!(
                    "Got {} route points from Garmin round-trip API",
                    route_coords.len()
                );
                Self::coords_to_geo_points(&route_coords, distance_m)
            }
            Err(e) => {
                info!(
                    "Garmin round-trip API failed ({}), falling back to circular route",
                    e
                );
                Self::generate_circular_route(lat, lng, distance_m)
            }
        };

        let num_points = geo_points.len();

        // Compute bounding box
        let mut min_lat = f64::MAX;
        let mut max_lat = f64::MIN;
        let mut min_lng = f64::MAX;
        let mut max_lng = f64::MIN;
        for pt in &geo_points {
            let plat = pt["latitude"].as_f64().unwrap_or(lat);
            let plng = pt["longitude"].as_f64().unwrap_or(lng);
            if plat < min_lat {
                min_lat = plat;
            }
            if plat > max_lat {
                max_lat = plat;
            }
            if plng < min_lng {
                min_lng = plng;
            }
            if plng > max_lng {
                max_lng = plng;
            }
        }

        let payload = serde_json::json!({
            "courseName": name,
            "activityTypePk": 1,
            "coordinateSystem": "WGS84",
            "sourceTypeId": 3,
            "rulePK": 2,
            "distanceMeter": distance_m,
            "elevationGainMeter": 0,
            "elevationLossMeter": 0,
            "openStreetMap": false,
            "hasTurnDetectionDisabled": false,
            "coursePoints": [],
            "geoPoints": geo_points,
            "courseLines": [{
                "distanceInMeters": distance_m,
                "sortOrder": 1,
                "numberOfPoints": num_points,
                "bearing": 0,
                "coordinateSystem": "WGS84",
                "points": null,
                "courseId": null
            }],
            "boundingBox": {
                "lowerLeft": { "lat": min_lat, "lng": min_lng },
                "upperRight": { "lat": max_lat, "lng": max_lng }
            },
            "startPoint": {
                "latitude": lat,
                "longitude": lng
            }
        });

        info!(
            "Creating course '{}' ({:.1} km, {} points) at ({:.5}, {:.5})",
            name,
            distance_m / 1000.0,
            num_points,
            lat,
            lng
        );

        self.api.create_course(&payload).await
    }

    /// Converts (lat, lng) coordinate pairs into Garmin geoPoint objects
    /// with cumulative distance.
    fn coords_to_geo_points(
        coords: &[(f64, f64)],
        total_distance_m: f64,
    ) -> Vec<serde_json::Value> {
        if coords.is_empty() {
            return Vec::new();
        }

        // Compute cumulative haversine distances between consecutive points
        let mut cumulative_distances = vec![0.0_f64];
        for i in 1..coords.len() {
            let d = haversine_distance(coords[i - 1].0, coords[i - 1].1, coords[i].0, coords[i].1);
            cumulative_distances.push(cumulative_distances[i - 1] + d);
        }
        let raw_total = *cumulative_distances.last().unwrap_or(&1.0);

        coords
            .iter()
            .enumerate()
            .map(|(i, (lat, lng))| {
                // Scale cumulative distance to match the requested total distance
                let dist = if raw_total > 0.0 {
                    cumulative_distances[i] / raw_total * total_distance_m
                } else {
                    0.0
                };
                serde_json::json!({
                    "latitude": lat,
                    "longitude": lng,
                    "distance": dist,
                    "elevation": 0.0,
                    "timestamp": null
                })
            })
            .collect()
    }

    /// Generates GPS points forming a circular loop of the given distance.
    /// Used as fallback when Garmin's round-trip route API is unavailable.
    fn generate_circular_route(lat: f64, lng: f64, distance_m: f64) -> Vec<serde_json::Value> {
        let num_points = 72; // every 5 degrees
        let radius_m = distance_m / (2.0 * std::f64::consts::PI);

        // Convert radius to approximate lat/lng deltas
        let lat_per_meter = 1.0 / 111_320.0;
        let lng_per_meter = 1.0 / (111_320.0 * lat.to_radians().cos());

        let mut points = Vec::with_capacity(num_points + 1);
        let circumference = distance_m;

        for i in 0..=num_points {
            let angle = (i as f64 / num_points as f64) * 2.0 * std::f64::consts::PI;
            let point_lat = lat + radius_m * angle.sin() * lat_per_meter;
            let point_lng = lng + radius_m * angle.cos() * lng_per_meter;
            let cumulative_distance = (i as f64 / num_points as f64) * circumference;

            points.push(serde_json::json!({
                "latitude": point_lat,
                "longitude": point_lng,
                "distance": cumulative_distance,
                "elevation": 0.0,
                "timestamp": null
            }));
        }

        points
    }

    /// Finds the start GPS coordinates from the most recent running activity.
    pub async fn get_last_run_start_location(&self) -> Option<(f64, f64)> {
        let activities = match self.api.get_activities(0, 50).await {
            Ok(acts) => acts,
            Err(_) => return None,
        };

        for act in &activities {
            let is_running = act
                .get_activity_type()
                .map(|t| {
                    let lower = t.to_lowercase();
                    lower.contains("run") || lower.contains("trail")
                })
                .unwrap_or(false);

            if !is_running {
                continue;
            }

            let start_lat = act
                .raw_fields
                .get("startLatitude")
                .and_then(|v| v.as_f64());
            let start_lng = act
                .raw_fields
                .get("startLongitude")
                .and_then(|v| v.as_f64());

            if let (Some(lat), Some(lng)) = (start_lat, start_lng) {
                if lat.abs() > 0.001 && lng.abs() > 0.001 {
                    return Some((lat, lng));
                }
            }
        }

        None
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
