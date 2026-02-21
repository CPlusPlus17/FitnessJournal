use axum::{
    extract::{DefaultBodyLimit, Request, State},
    http::{header, HeaderMap, HeaderName, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;

use crate::coaching::Coach;
use crate::db::Database;
use crate::garmin_client::GarminClient;

const MAX_CHAT_INPUT_LEN: usize = 4_000;

#[derive(Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct ChatInput {
    pub content: String,
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
    database: Arc<Mutex<Database>>,
    garmin_client: Arc<GarminClient>,
    coach: Arc<Coach>,
    api_auth_token: Option<String>,
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

fn env_usize(var_name: &str, default_value: usize) -> usize {
    std::env::var(var_name)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default_value)
}

fn cors_origins() -> Vec<HeaderValue> {
    let raw = std::env::var("CORS_ALLOWED_ORIGINS")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());

    let mut origins = Vec::new();
    for origin in raw.split(',') {
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

async fn auth_middleware(State(state): State<ApiState>, request: Request, next: Next) -> Response {
    if request.method() == Method::OPTIONS {
        return next.run(request).await;
    }

    if let Some(expected_token) = &state.api_auth_token {
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
    database: Arc<Mutex<Database>>,
    garmin_client: Arc<GarminClient>,
    coach: Arc<Coach>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let auth_token = std::env::var("API_AUTH_TOKEN")
        .ok()
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty());

    let state = ApiState {
        database,
        garmin_client,
        coach,
        api_auth_token: auth_token,
        chat_limiter: Arc::new(Mutex::new(SlidingWindowLimiter::new(
            env_usize("CHAT_RATE_LIMIT_PER_MINUTE", 30),
            Duration::from_secs(60),
        ))),
        generate_limiter: Arc::new(Mutex::new(SlidingWindowLimiter::new(
            env_usize("GENERATE_RATE_LIMIT_PER_HOUR", 6),
            Duration::from_secs(60 * 60),
        ))),
    };

    let cors = CorsLayer::new()
        .allow_origin(cors_origins())
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            HeaderName::from_static("x-api-token"),
        ]);

    let app = Router::new()
        .route("/api/progression", get(get_progression))
        .route("/api/recovery", get(get_recovery))
        .route("/api/workouts/today", get(get_today_workouts))
        .route("/api/workouts/upcoming", get(get_upcoming_workouts))
        .route("/api/generate", axum::routing::post(trigger_generate))
        .route("/api/muscle_heatmap", get(get_muscle_heatmap))
        .route("/api/chat", get(get_chat).post(post_chat))
        .with_state(state.clone())
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(middleware::from_fn_with_state(state, auth_middleware))
        .layer(cors);

    let bind_addr = std::env::var("API_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:3001".to_string());
    let addr: SocketAddr = bind_addr.parse().map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Invalid API_BIND_ADDR '{}': {}", bind_addr, e),
        )
    })?;

    println!("API Server running at http://{}", addr);

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
        state.garmin_client.clone(),
        state.coach.clone(),
        state.database.clone(),
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
    let history = db.get_ai_chat_history().unwrap_or_default();
    let mut resp = Vec::with_capacity(history.len());
    for (role, content) in history {
        resp.push(ChatMessage { role, content });
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

    let gemini_key = std::env::var("GEMINI_API_KEY").map_err(|_| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "status": "error",
                "message": "No API key"
            })),
        )
    })?;

    let ai_client = crate::ai_client::AiClient::new(gemini_key);

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

    let history = state
        .database
        .lock()
        .await
        .get_ai_chat_history()
        .unwrap_or_default();

    match ai_client.chat_with_history(&history).await {
        Ok(response) => {
            let db = state.database.lock().await;
            if let Err(e) = db.add_ai_chat_message("model", &response) {
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
