use axum::{
    extract::{rejection::JsonRejection, DefaultBodyLimit, Request, State},
    http::{header, HeaderMap, HeaderName, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, VecDeque},
    net::SocketAddr,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use tracing::{error, info};

use crate::coaching::Coach;
use crate::db::Database;
use crate::garmin_client::GarminClient;

const MAX_CHAT_INPUT_LEN: usize = 65_536;
const MAX_PROFILE_NAME_LEN: usize = 64;
const MAX_PROFILE_ITEMS: usize = 64;
const MAX_PROFILE_ITEM_LEN: usize = 256;
fn profiles_path() -> String {
    std::env::var("PROFILES_PATH").unwrap_or_else(|_| "data/profiles.json".to_string())
}

#[derive(Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub created_at: u64,
}

#[derive(Deserialize)]
pub struct ChatInput {
    pub content: String,
}

#[derive(Deserialize)]
pub struct AnalyzeActivityInput {
    pub activity: serde_json::Value,
}

#[derive(Deserialize)]
pub struct PredictDurationInput {
    pub title: Option<String>,
    pub sport: Option<String>,
    pub description: Option<String>,
}

#[derive(Deserialize)]
pub struct AnalyzeUpcomingInput {
    pub workout: crate::models::ScheduledWorkout,
}

#[derive(Deserialize)]
pub struct CreateCourseInput {
    pub workout: crate::models::ScheduledWorkout,
    pub start_latitude: Option<f64>,
    pub start_longitude: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileConfigPayload {
    #[serde(default)]
    goals: Vec<String>,
    #[serde(default)]
    constraints: Vec<String>,
    #[serde(default)]
    available_equipment: Vec<String>,
    #[serde(default)]
    auto_analyze_sports: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfilesPayload {
    active_profile: String,
    #[serde(default)]
    profiles: BTreeMap<String, ProfileConfigPayload>,
}

#[derive(Debug)]
struct SlidingWindowLimiter {
    max_requests: usize,
    window: Duration,
    hits: VecDeque<Instant>,
}

impl SlidingWindowLimiter {
    fn new(max_requests: usize, window: Duration) -> Self {
        Self {
            max_requests,
            window,
            hits: VecDeque::new(),
        }
    }

    fn allow(&mut self) -> bool {
        let now = Instant::now();
        while let Some(oldest) = self.hits.front() {
            if now.duration_since(*oldest) > self.window {
                self.hits.pop_front();
            } else {
                break;
            }
        }

        if self.hits.len() >= self.max_requests {
            return false;
        }

        self.hits.push_back(now);
        true
    }
}

#[derive(Clone)]
pub struct ApiState {
    pub config: Arc<crate::config::AppConfig>,
    database: Arc<Mutex<Database>>,
    garmin_client: Arc<GarminClient>,
    coach: Arc<Coach>,
    chat_limiter: Arc<Mutex<SlidingWindowLimiter>>,
    generate_limiter: Arc<Mutex<SlidingWindowLimiter>>,
}

#[derive(Serialize)]
pub struct TrendPoint {
    pub weight: f64,
    pub reps: i32,
    pub date: String,
}

#[derive(Serialize)]
pub struct ProgressionResponse {
    pub exercise_name: String,
    pub max_weight: f64,
    pub reps: i32,
    pub date: String,
    pub history: Vec<TrendPoint>,
}

#[derive(Serialize)]
pub struct WeeklyDeltaResponse {
    pub exercise_name: String,
    pub this_week_weight: f64,
    pub this_week_reps: i32,
    pub last_week_weight: f64,
    pub last_week_reps: i32,
}

#[derive(Serialize)]
pub struct TodayWorkoutsResponse {
    pub done: Vec<crate::models::GarminActivity>,
    pub planned: Vec<crate::models::ScheduledWorkout>,
}

#[derive(Serialize)]
pub struct RecoveryResponse {
    pub body_battery: Option<i32>,
    pub sleep_score: Option<i32>,
    pub training_readiness: Option<i32>,
    pub hrv_status: Option<String>,
    pub hrv_weekly_avg: Option<i32>,
    pub hrv_last_night_avg: Option<i32>,
    pub rhr_trend: Vec<i32>,
}

fn cors_origins(raw_origins: &str) -> Vec<HeaderValue> {
    let mut origins = Vec::new();
    for origin in raw_origins.split(',') {
        let trimmed = origin.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Ok(header_value) = HeaderValue::from_str(trimmed) {
            origins.push(header_value);
        }
    }

    if origins.is_empty() {
        origins.push(HeaderValue::from_static("http://localhost:3000"));
    }

    origins
}

fn has_valid_api_token(headers: &HeaderMap, expected: &str) -> bool {
    if let Some(value) = headers.get("x-api-token") {
        if let Ok(token) = value.to_str() {
            if token == expected {
                return true;
            }
        }
    }

    if let Some(value) = headers.get(header::AUTHORIZATION) {
        if let Ok(raw) = value.to_str() {
            if let Some(token) = raw.strip_prefix("Bearer ") {
                return token == expected;
            }
        }
    }

    false
}

fn error_response(status: StatusCode, message: &str) -> (StatusCode, Json<serde_json::Value>) {
    (
        status,
        Json(serde_json::json!({
            "status": "error",
            "message": message
        })),
    )
}

fn normalize_profile_list(
    values: &[String],
    profile_name: &str,
    field_name: &str,
) -> Result<Vec<String>, String> {
    if values.len() > MAX_PROFILE_ITEMS {
        return Err(format!(
            "Profile '{}' has too many '{}' entries (max {}).",
            profile_name, field_name, MAX_PROFILE_ITEMS
        ));
    }

    let mut normalized = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.chars().count() > MAX_PROFILE_ITEM_LEN {
            return Err(format!(
                "Profile '{}' has an entry in '{}' that exceeds {} characters.",
                profile_name, field_name, MAX_PROFILE_ITEM_LEN
            ));
        }

        normalized.push(trimmed.to_string());
    }

    Ok(normalized)
}

fn validate_profiles_payload(payload: ProfilesPayload) -> Result<ProfilesPayload, String> {
    let active_profile = payload.active_profile.trim();
    if active_profile.is_empty() {
        return Err("active_profile cannot be empty.".to_string());
    }
    if active_profile.chars().count() > MAX_PROFILE_NAME_LEN {
        return Err(format!(
            "active_profile exceeds {} characters.",
            MAX_PROFILE_NAME_LEN
        ));
    }
    if payload.profiles.is_empty() {
        return Err("profiles must include at least one profile.".to_string());
    }

    let mut normalized_profiles = BTreeMap::new();
    for (raw_name, profile) in payload.profiles {
        let profile_name = raw_name.trim();
        if profile_name.is_empty() {
            return Err("Profile names cannot be empty.".to_string());
        }
        if profile_name.chars().count() > MAX_PROFILE_NAME_LEN {
            return Err(format!(
                "Profile name '{}' exceeds {} characters.",
                profile_name, MAX_PROFILE_NAME_LEN
            ));
        }
        if normalized_profiles.contains_key(profile_name) {
            return Err(format!("Duplicate profile name '{}'.", profile_name));
        }

        let normalized_profile = ProfileConfigPayload {
            goals: normalize_profile_list(&profile.goals, profile_name, "goals")?,
            constraints: normalize_profile_list(&profile.constraints, profile_name, "constraints")?,
            available_equipment: normalize_profile_list(
                &profile.available_equipment,
                profile_name,
                "available_equipment",
            )?,
            auto_analyze_sports: normalize_profile_list(
                &profile.auto_analyze_sports,
                profile_name,
                "auto_analyze_sports",
            )?,
        };

        normalized_profiles.insert(profile_name.to_string(), normalized_profile);
    }

    if !normalized_profiles.contains_key(active_profile) {
        return Err(format!(
            "active_profile '{}' must reference an existing profile.",
            active_profile
        ));
    }

    Ok(ProfilesPayload {
        active_profile: active_profile.to_string(),
        profiles: normalized_profiles,
    })
}

fn write_file_atomically(path: &Path, content: &str) -> std::io::Result<()> {
    let mut tmp_path = path.to_path_buf();
    tmp_path.set_extension("json.tmp");

    std::fs::write(&tmp_path, content)?;
    if let Err(err) = std::fs::rename(&tmp_path, path) {
        // Docker file bind mounts can reject atomic replace with EBUSY/EXDEV.
        // In that case we fall back to direct write to preserve functionality.
        let needs_fallback = matches!(err.raw_os_error(), Some(16 | 18));
        if needs_fallback {
            error!(
                "Atomic replace failed for {} ({}). Falling back to direct write.",
                path.display(),
                err
            );
            std::fs::write(path, content)?;
            let _ = std::fs::remove_file(&tmp_path);
            return Ok(());
        }

        let _ = std::fs::remove_file(&tmp_path);
        return Err(err);
    }

    Ok(())
}

async fn auth_middleware(State(state): State<ApiState>, request: Request, next: Next) -> Response {
    if request.method() == Method::OPTIONS {
        return next.run(request).await;
    }

    if let Some(expected_token) = &state.config.api_auth_token {
        if !has_valid_api_token(request.headers(), expected_token) {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "status": "error",
                    "message": "Unauthorized"
                })),
            )
                .into_response();
        }
    }

    next.run(request).await
}

pub async fn run_server(
    config: Arc<crate::config::AppConfig>,
    database: Arc<Mutex<Database>>,
    garmin_client: Arc<GarminClient>,
    coach: Arc<Coach>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let state = ApiState {
        chat_limiter: Arc::new(Mutex::new(SlidingWindowLimiter::new(
            config.chat_rate_limit_per_minute,
            Duration::from_secs(60),
        ))),
        generate_limiter: Arc::new(Mutex::new(SlidingWindowLimiter::new(
            config.generate_rate_limit_per_hour,
            Duration::from_secs(60 * 60),
        ))),
        config: config.clone(),
        database,
        garmin_client,
        coach,
    };

    let cors = CorsLayer::new()
        .allow_origin(cors_origins(&config.cors_allowed_origins))
        .allow_methods([Method::GET, Method::POST, Method::PUT])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            HeaderName::from_static("x-api-token"),
        ]);

    let app = Router::new()
        .route("/api/progression", get(get_progression))
        .route("/api/progression/deltas", get(get_weekly_deltas))
        .route("/api/recovery", get(get_recovery))
        .route("/api/recovery/history", get(get_recovery_history))
        .route("/api/workouts/today", get(get_today_workouts))
        .route("/api/workouts/upcoming", get(get_upcoming_workouts))
        .route("/api/activities/week", get(get_week_activities))
        .route("/api/force-pull", axum::routing::post(force_pull_data))
        .route("/api/generate", axum::routing::post(trigger_generate))
        .route(
            "/api/predict_duration",
            axum::routing::post(predict_duration),
        )
        .route("/api/analyze", axum::routing::post(analyze_activity))
        .route(
            "/api/analyze/upcoming",
            axum::routing::post(analyze_upcoming_event),
        )
        .route(
            "/api/course/create",
            axum::routing::post(create_course),
        )
        .route("/api/muscle_heatmap", get(get_muscle_heatmap))
        .route("/api/chat", get(get_chat).post(post_chat))
        .route("/api/profiles", get(get_profiles).put(update_profiles))
        .with_state(state.clone())
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(middleware::from_fn_with_state(state, auth_middleware))
        .layer(cors);

    let addr: SocketAddr = config.api_bind_addr.parse().map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Invalid API_BIND_ADDR '{}': {}", config.api_bind_addr, e),
        )
    })?;

    info!("API Server running at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn trigger_generate(
    State(state): State<ApiState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    if !state.generate_limiter.lock().await.allow() {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "status": "error",
                "message": "Rate limit exceeded for /api/generate"
            })),
        ));
    }

    match crate::run_coach_pipeline(
        state.config.clone(),
        state.garmin_client.clone(),
        state.coach.clone(),
        state.database.clone(),
        true,
    )
    .await
    {
        Ok(_) => Ok(Json(serde_json::json!({
            "status": "success",
            "message": "Workouts generated and pushed to Garmin"
        }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "status": "error",
                "message": e.to_string()
            })),
        )),
    }
}

async fn get_chat(State(state): State<ApiState>) -> Json<Vec<ChatMessage>> {
    let db = state.database.lock().await;
    let history = db.get_coach_briefs().unwrap_or_default();
    let mut resp = Vec::with_capacity(history.len() * 2);
    for (prompt, response, created_at) in history {
        resp.push(ChatMessage {
            role: "user".to_string(),
            content: prompt,
            created_at,
        });
        resp.push(ChatMessage {
            role: "model".to_string(),
            content: response,
            created_at,
        });
    }
    Json(resp)
}

async fn post_chat(
    State(state): State<ApiState>,
    Json(input): Json<ChatInput>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    if !state.chat_limiter.lock().await.allow() {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "status": "error",
                "message": "Rate limit exceeded for /api/chat"
            })),
        ));
    }

    let content = input.content.trim();
    if content.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "status": "error",
                "message": "Chat content cannot be empty"
            })),
        ));
    }

    if content.chars().count() > MAX_CHAT_INPUT_LEN {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(serde_json::json!({
                "status": "error",
                "message": format!("Chat content exceeds {} characters", MAX_CHAT_INPUT_LEN)
            })),
        ));
    }

    let gemini_key = &state.config.gemini_api_key;
    if gemini_key.is_empty() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "status": "error",
                "message": "No API key"
            })),
        ));
    }

    let gemini_model =
        std::env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-3-flash-preview".to_string());

    {
        let db = state.database.lock().await;
        if let Err(e) = db.add_ai_chat_message("user", content) {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "status": "error",
                    "message": format!("Failed to save input: {}", e)
                })),
            ));
        }
    }

    let ai_client = crate::ai_client::AiClient::new(gemini_key.clone(), gemini_model);

    let history_pairs = state
        .database
        .lock()
        .await
        .get_coach_briefs()
        .unwrap_or_default();

    let mut history = Vec::with_capacity(history_pairs.len() * 2 + 1);
    for (prompt, response, created_at) in history_pairs {
        history.push(("user".to_string(), prompt, created_at));
        history.push(("model".to_string(), response, created_at));
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    history.push(("user".to_string(), content.to_string(), now));

    match ai_client.chat_with_history(&history, None).await {
        Ok(response) => {
            let db = state.database.lock().await;
            if let Err(e) = db.add_coach_brief(content, &response) {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "status": "error",
                        "message": format!("Failed to save model response: {}", e)
                    })),
                ));
            }

            Ok(Json(serde_json::json!({
                "status": "success",
                "message": "Responded"
            })))
        }
        Err(e) => Err((
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "status": "error",
                "message": e.to_string()
            })),
        )),
    }
}

async fn get_progression(State(state): State<ApiState>) -> Json<Vec<ProgressionResponse>> {
    let db = state.database.lock().await;
    let history = db.get_progression_history_raw().unwrap_or_default();

    let mut response = Vec::with_capacity(history.len());
    for (name, weight, reps, date, trend_history) in history {
        let history_points = trend_history
            .into_iter()
            .map(|(w, r, d)| TrendPoint {
                weight: w,
                reps: r,
                date: d,
            })
            .collect();

        response.push(ProgressionResponse {
            exercise_name: name,
            max_weight: weight,
            reps,
            date,
            history: history_points,
        });
    }

    Json(response)
}

async fn get_muscle_heatmap(
    State(state): State<ApiState>,
) -> Json<Vec<crate::models::ExerciseMuscleMap>> {
    let db = state.database.lock().await;
    let heatmap = db.get_recent_muscle_heatmap(14).unwrap_or_default();
    Json(heatmap)
}

async fn get_recovery(State(state): State<ApiState>) -> Json<RecoveryResponse> {
    let mut response = RecoveryResponse {
        body_battery: None,
        sleep_score: None,
        training_readiness: None,
        hrv_status: None,
        hrv_weekly_avg: None,
        hrv_last_night_avg: None,
        rhr_trend: Vec::new(),
    };

    if let Ok(data) = state.garmin_client.fetch_data().await {
        if let Some(metrics) = data.recovery_metrics {
            response.body_battery = metrics.current_body_battery;
            response.sleep_score = metrics.sleep_score;
            response.training_readiness = metrics.training_readiness;
            response.hrv_status = metrics.hrv_status;
            response.hrv_weekly_avg = metrics.hrv_weekly_avg;
            response.hrv_last_night_avg = metrics.hrv_last_night_avg;
            response.rhr_trend = metrics.rhr_trend;
        }
    }

    Json(response)
}

async fn get_recovery_history(
    State(state): State<ApiState>,
) -> Json<Vec<crate::db::RecoveryHistoryEntry>> {
    let db = state.database.lock().await;
    // Fetch the last 30 days of recovery history to render on the dashboard charts
    let history = db.get_recovery_history(30).unwrap_or_default();
    Json(history)
}

async fn get_today_workouts(State(state): State<ApiState>) -> Json<TodayWorkoutsResponse> {
    let mut response = TodayWorkoutsResponse {
        done: Vec::new(),
        planned: Vec::new(),
    };

    let today_prefix = chrono::Local::now().format("%Y-%m-%d").to_string();

    if let Ok(data) = state.garmin_client.fetch_data().await {
        response.done = data
            .activities
            .into_iter()
            .filter(|a| a.start_time.starts_with(&today_prefix))
            .collect();

        response.planned = data
            .scheduled_workouts
            .into_iter()
            .filter(|w| w.date.starts_with(&today_prefix))
            .collect();
    }

    Json(response)
}

async fn get_upcoming_workouts(
    State(state): State<ApiState>,
) -> Json<Vec<crate::models::ScheduledWorkout>> {
    let mut planned = Vec::new();
    let today_prefix = chrono::Local::now().format("%Y-%m-%d").to_string();

    if let Ok(data) = state.garmin_client.fetch_data().await {
        planned = data
            .scheduled_workouts
            .into_iter()
            .filter(|w| w.date >= today_prefix)
            .collect();
    }

    planned.sort_by(|a, b| a.date.cmp(&b.date));
    Json(planned)
}

async fn get_week_activities(
    State(state): State<ApiState>,
) -> Json<Vec<crate::models::GarminActivity>> {
    let today = chrono::Local::now().date_naive();
    let week_ago = (today - chrono::Duration::days(7))
        .format("%Y-%m-%d")
        .to_string();

    if let Ok(data) = state.garmin_client.fetch_data().await {
        let mut activities: Vec<_> = data
            .activities
            .into_iter()
            .filter(|a| a.start_time >= week_ago)
            .collect();
        activities.sort_by(|a, b| a.start_time.cmp(&b.start_time));
        return Json(activities);
    }

    Json(Vec::new())
}

async fn get_profiles() -> Result<Json<ProfilesPayload>, (StatusCode, Json<serde_json::Value>)> {
    let path = profiles_path();
    let data = std::fs::read_to_string(&path).map_err(|err| {
        error!("Failed to read {}: {}", path, err);
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Profiles configuration is unavailable.",
        )
    })?;

    let parsed = serde_json::from_str::<ProfilesPayload>(&data).map_err(|err| {
        error!("Failed to parse {}: {}", path, err);
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Profiles configuration is invalid.",
        )
    })?;

    let validated = validate_profiles_payload(parsed).map_err(|err| {
        error!("Validation failed for {}: {}", path, err);
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Profiles configuration is invalid.",
        )
    })?;

    Ok(Json(validated))
}

async fn update_profiles(
    payload: Result<Json<ProfilesPayload>, JsonRejection>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let Json(payload) = payload.map_err(|err| {
        error!("Rejected invalid profiles payload: {}", err);
        error_response(StatusCode::BAD_REQUEST, "Invalid profiles payload.")
    })?;

    let validated = validate_profiles_payload(payload)
        .map_err(|err| error_response(StatusCode::BAD_REQUEST, &err))?;

    let path = profiles_path();
    let mut json_str = serde_json::to_string_pretty(&validated).map_err(|err| {
        error!("Failed to serialize {} payload: {}", path, err);
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to persist profiles configuration.",
        )
    })?;
    json_str.push('\n');

    write_file_atomically(Path::new(&path), &json_str).map_err(|err| {
        error!("Failed to atomically write {}: {}", path, err);
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to persist profiles configuration.",
        )
    })?;

    Ok(Json(serde_json::json!({
        "status": "success",
        "message": "Profiles updated"
    })))
}

async fn predict_duration(
    State(state): State<ApiState>,
    Json(input): Json<PredictDurationInput>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let title = input.title.unwrap_or_default();
    let sport = input.sport.unwrap_or_default();
    let cache_key = format!("{}|{}", title, sport);

    {
        let db = state.database.lock().await;
        if let Ok(Some(duration)) = db.get_predicted_duration(&cache_key) {
            return Ok(Json(serde_json::json!({ "duration": duration })));
        }
    }

    let gemini_key = &state.config.gemini_api_key;
    if gemini_key.is_empty() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "status": "error",
                "message": "GEMINI_API_KEY not configured"
            })),
        ));
    }

    let gemini_model =
        std::env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-3-flash-preview".to_string());
    let ai_client = crate::ai_client::AiClient::new(gemini_key.clone(), gemini_model);
    let prompt = format!(
        "Predict the duration in minutes for this workout. Take into account conventional durations for these types of workouts. Return only a plain integer representing minutes, and nothing else (no units, no markdown). If you cannot predict or it's unknown, return 45.\nTitle: {}\nSport: {}\nDescription: {}",
        title, sport, input.description.unwrap_or_default()
    );

    match ai_client.generate_workout(&prompt).await {
        Ok(text) => {
            let parsed = text.trim().parse::<i32>().unwrap_or(45);
            {
                let db = state.database.lock().await;
                let _ = db.set_predicted_duration(&cache_key, parsed);
            }

            Ok(Json(serde_json::json!({
                "duration": parsed
            })))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "status": "error",
                "message": e.to_string()
            })),
        )),
    }
}

async fn analyze_activity(
    State(state): State<ApiState>,
    Json(input): Json<AnalyzeActivityInput>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let garmin_act =
        serde_json::from_value::<crate::models::GarminActivity>(input.activity.clone()).ok();
    let activity_id = garmin_act.as_ref().and_then(|a| a.id);
    let start_time = garmin_act
        .as_ref()
        .map(|a| a.start_time.clone())
        .unwrap_or_default();

    // Check DB first
    if let Some(id) = activity_id {
        let db = state.database.lock().await;
        if let Ok(Some(existing_analysis)) = db.get_activity_analysis(id) {
            return Ok(Json(serde_json::json!({
                "analysis": existing_analysis
            })));
        }
    }

    let gemini_key = &state.config.gemini_api_key;
    if gemini_key.is_empty() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "status": "error",
                "message": "GEMINI_API_KEY not configured"
            })),
        ));
    }

    let gemini_model =
        std::env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-3-flash-preview".to_string());
    let ai_client = crate::ai_client::AiClient::new(gemini_key.clone(), gemini_model);
    let prompt = format!(
        "Please provide an in-depth analysis of this completed fitness activity. Be encouraging but highly analytical.\n\nYou have been provided with the complete, raw JSON payload direct from Garmin. It contains many undocumented fields, extra metrics, recovery data, elevation, stress, cadence, temperatures, or detailed exercise sets.\n\nPlease actively hunt through this raw JSON and surface interesting insights, anomalies, or performance correlations that wouldn't be obvious from just the basic time/distance metrics. Explain what these deeper metrics mean for the athlete's progress.\n\nHere is the raw Garmin activity data in JSON format:\n\n{}",
        serde_json::to_string(&input.activity).unwrap_or_default()
    );

    match ai_client.generate_workout(&prompt).await {
        Ok(text) => {
            // Save to DB
            if let Some(id) = activity_id {
                let db = state.database.lock().await;
                let _ = db.save_activity_analysis(id, &start_time, &text);
            }
            Ok(Json(serde_json::json!({
                "analysis": text
            })))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "status": "error",
                "message": e.to_string()
            })),
        )),
    }
}

async fn force_pull_data(
    State(state): State<ApiState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    {
        let db = state.database.lock().await;
        if let Err(e) = db.clear_garmin_cache() {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "status": "error",
                    "message": format!("Failed to clear database garmin cache: {}", e)
                })),
            ));
        }
    }

    match state.garmin_client.fetch_data().await {
        Ok(_) => Ok(Json(serde_json::json!({
            "status": "success",
            "message": "Data successfully force-pulled from Garmin."
        }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "status": "error",
                "message": e.to_string()
            })),
        )),
    }
}

async fn get_weekly_deltas(State(state): State<ApiState>) -> Json<Vec<WeeklyDeltaResponse>> {
    use chrono::Datelike;

    let week_start_chrono = crate::config::parse_weekday(&state.config.week_start_day);
    let today = chrono::Local::now().date_naive();

    let days_since_start = (today.weekday().num_days_from_monday() as i64
        - week_start_chrono.num_days_from_monday() as i64
        + 7)
        % 7;
    let this_week_start = today - chrono::Duration::days(days_since_start);
    let last_week_start = this_week_start - chrono::Duration::days(7);

    let this_week_start_str = this_week_start.format("%Y-%m-%d").to_string();
    let last_week_start_str = last_week_start.format("%Y-%m-%d").to_string();

    let db = state.database.lock().await;
    let deltas = db
        .get_weekly_progression_deltas(&this_week_start_str, &last_week_start_str)
        .unwrap_or_default();

    let response: Vec<WeeklyDeltaResponse> = deltas
        .into_iter()
        .map(|(name, tw, tr, lw, lr)| WeeklyDeltaResponse {
            exercise_name: name,
            this_week_weight: tw,
            this_week_reps: tr,
            last_week_weight: lw,
            last_week_reps: lr,
        })
        .collect();

    Json(response)
}

async fn analyze_upcoming_event(
    State(state): State<ApiState>,
    Json(input): Json<AnalyzeUpcomingInput>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let workout = input.workout;
    let gemini_key = &state.config.gemini_api_key;
    if gemini_key.is_empty() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "status": "error",
                "message": "GEMINI_API_KEY not configured"
            })),
        ));
    }

    let gemini_model =
        std::env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-3-flash-preview".to_string());
    let ai_client = crate::ai_client::AiClient::new(gemini_key.clone(), gemini_model);

    // Provide context
    let mut context_str = String::new();
    if let Ok(data) = state.garmin_client.fetch_data().await {
        if let Some(metrics) = data.recovery_metrics {
            if let Ok(json) = serde_json::to_string(&metrics) {
                context_str.push_str(&format!("Current Recovery Metrics: {}\n\n", json));
            }
        }
        if let Some(profile) = data.user_profile {
            if let Ok(json) = serde_json::to_string(&profile) {
                context_str.push_str(&format!("User Profile: {}\n\n", json));
            }
        }
        if let Some(max_metrics) = data.max_metrics {
            if let Ok(json) = serde_json::to_string(&max_metrics) {
                context_str.push_str(&format!("Max Metrics: {}\n\n", json));
            }
        }

        let recent_activities: Vec<_> = data.activities.iter().take(15).collect();
        if let Ok(json) = serde_json::to_string(&recent_activities) {
            context_str.push_str(&format!("Recent 15 Activities: {}\n\n", json));
        }

        let upcoming_workouts: Vec<_> = data
            .scheduled_workouts
            .iter()
            .filter(|w| w.date < workout.date)
            .collect();
        if let Ok(json) = serde_json::to_string(&upcoming_workouts) {
            context_str.push_str(&format!("Upcoming Workouts before event: {}\n\n", json));
        }
    }

    let workout_json = serde_json::to_string(&workout).unwrap_or_default();

    let cache_key = format!(
        "{}|{}|{}",
        workout.date,
        workout.title.as_deref().unwrap_or_default(),
        workout.sport.as_deref().unwrap_or_default()
    );

    // Check DB first
    {
        let db = state.database.lock().await;
        if let Ok(Some(existing_analysis)) = db.get_upcoming_analysis(&cache_key) {
            return Ok(Json(serde_json::json!({
                "analysis": existing_analysis
            })));
        }
    }

    let prompt = format!(
        "Please provide an in-depth analysis of this upcoming primary event/race.
You have been provided with the event details as well as the athlete's current state of preparation.
Evaluate their current state of preparation, projected readiness for the event, what they can expect to achieve based on their recent performance and metrics (VO2 Max, Recent Activities, Recovery, etc.), and provide any strategic race-day advice or tapering suggestions.
CRITICAL INSTRUCTION: DO NOT generate, suggest, or output any new workouts or JSON workout blocks. Only provide the text analysis.

Here is the upcoming event:
{}

Here is the athlete's current state of preparation context:
{}",
        workout_json, context_str
    );

    match ai_client.generate_workout(&prompt).await {
        Ok(text) => {
            {
                let db = state.database.lock().await;
                let _ = db.set_upcoming_analysis(&cache_key, &text);
            }
            Ok(Json(serde_json::json!({
                "analysis": text
            })))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "status": "error",
                "message": e.to_string()
            })),
        )),
    }
}

async fn create_course(
    State(state): State<ApiState>,
    Json(input): Json<CreateCourseInput>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    if !state.generate_limiter.lock().await.allow() {
        return Err(error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "Rate limit exceeded for /api/course/create",
        ));
    }

    let workout = &input.workout;

    // Resolve distance
    let distance_m = resolve_course_distance(&state, workout).await.map_err(|e| {
        error_response(
            StatusCode::BAD_REQUEST,
            &format!("Could not determine distance: {}", e),
        )
    })?;

    // Resolve start coordinates
    let (lat, lng) = resolve_course_coordinates(&state, &input).await.map_err(|e| {
        error_response(
            StatusCode::BAD_REQUEST,
            &format!("Could not determine start coordinates: {}", e),
        )
    })?;

    let course_name = format!(
        "FJ-AI: {} {:.1}km Loop",
        workout.title.as_deref().unwrap_or("Run"),
        distance_m / 1000.0
    );

    match state
        .garmin_client
        .create_course_for_workout(&course_name, distance_m, lat, lng)
        .await
    {
        Ok(result) => {
            let course_id = result.get("courseId").and_then(|v| v.as_i64());
            Ok(Json(serde_json::json!({
                "status": "success",
                "courseName": course_name,
                "courseId": course_id,
                "distanceKm": distance_m / 1000.0
            })))
        }
        Err(e) => Err(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to create course: {}", e),
        )),
    }
}

async fn resolve_course_distance(
    state: &ApiState,
    workout: &crate::models::ScheduledWorkout,
) -> Result<f64, String> {
    // 1. Explicit distance on workout
    if let Some(d) = workout.distance {
        if d > 0.0 {
            return Ok(d);
        }
    }

    // 2. Adaptive details estimated distance
    if let Some(ref details) = workout.adaptive_details {
        if let Some(d) = details
            .get("estimatedDistanceInMeters")
            .and_then(|v| v.as_f64())
        {
            if d > 0.0 {
                return Ok(d);
            }
        }
    }

    // 3. AI estimation via Gemini
    let gemini_key = &state.config.gemini_api_key;
    if gemini_key.is_empty() {
        return Err("No distance available and GEMINI_API_KEY not configured".to_string());
    }

    let gemini_model =
        std::env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-3-flash-preview".to_string());
    let ai_client = crate::ai_client::AiClient::new(gemini_key.clone(), gemini_model);

    let workout_json = serde_json::to_string(workout).unwrap_or_default();
    let prompt = format!(
        "Estimate the distance in meters for this run workout. Return ONLY a plain integer representing meters, nothing else.\n\n{}",
        workout_json
    );

    match ai_client.generate_workout(&prompt).await {
        Ok(text) => {
            let parsed = text.trim().parse::<f64>().map_err(|_| {
                format!("AI returned non-numeric distance: {}", text.trim())
            })?;
            if parsed > 0.0 {
                Ok(parsed)
            } else {
                Err("AI estimated zero or negative distance".to_string())
            }
        }
        Err(e) => Err(format!("AI estimation failed: {}", e)),
    }
}

async fn resolve_course_coordinates(
    state: &ApiState,
    input: &CreateCourseInput,
) -> Result<(f64, f64), String> {
    // 1. Explicit from request body
    if let (Some(lat), Some(lng)) = (input.start_latitude, input.start_longitude) {
        if lat.abs() > 0.001 && lng.abs() > 0.001 {
            return Ok((lat, lng));
        }
    }

    // 2. Config defaults
    if let (Some(lat), Some(lng)) = (
        state.config.default_start_latitude,
        state.config.default_start_longitude,
    ) {
        if lat.abs() > 0.001 && lng.abs() > 0.001 {
            return Ok((lat, lng));
        }
    }

    // 3. Last GPS running activity
    if let Some((lat, lng)) = state.garmin_client.get_last_run_start_location().await {
        return Ok((lat, lng));
    }

    Err("No start coordinates available. Provide start_latitude/start_longitude, configure default_start_latitude/longitude, or complete a GPS run activity.".to_string())
}
