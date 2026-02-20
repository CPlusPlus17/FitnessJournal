use axum::{extract::State, routing::get, Json, Router};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};

use crate::coaching::Coach;
use crate::db::Database;
use crate::garmin_client::GarminClient;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct ChatInput {
    pub content: String,
}

#[derive(Clone)]
pub struct ApiState {
    pub database: Arc<Mutex<Database>>,
    pub garmin_client: Arc<GarminClient>,
    pub coach: Arc<Coach>,
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

pub async fn run_server(
    database: Arc<Mutex<Database>>,
    garmin_client: Arc<GarminClient>,
    coach: Arc<Coach>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let state = ApiState {
        database,
        garmin_client,
        coach,
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/progression", get(get_progression))
        .route("/api/recovery", get(get_recovery))
        .route("/api/workouts/today", get(get_today_workouts))
        .route("/api/workouts/upcoming", get(get_upcoming_workouts))
        .route("/api/generate", axum::routing::post(trigger_generate))
        .route("/api/muscle_heatmap", get(get_muscle_heatmap))
        .route("/api/chat", get(get_chat).post(post_chat))
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3001));
    println!("API Server running at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn trigger_generate(State(state): State<ApiState>) -> Json<serde_json::Value> {
    match crate::run_coach_pipeline(
        state.garmin_client.clone(),
        state.coach.clone(),
        state.database.clone(),
    )
    .await
    {
        Ok(_) => Json(
            serde_json::json!({ "status": "success", "message": "Workouts generated and pushed to Garmin" }),
        ),
        Err(e) => Json(serde_json::json!({ "status": "error", "message": e.to_string() })),
    }
}

async fn get_chat(State(state): State<ApiState>) -> Json<Vec<ChatMessage>> {
    let db = state.database.lock().await;
    let history = db.get_ai_chat_history().unwrap_or_default();
    let mut resp = Vec::new();
    for (role, content) in history {
        resp.push(ChatMessage { role, content });
    }
    Json(resp)
}

async fn post_chat(
    State(state): State<ApiState>,
    Json(input): Json<ChatInput>,
) -> Json<serde_json::Value> {
    if let Ok(gemini_key) = std::env::var("GEMINI_API_KEY") {
        let ai_client = crate::ai_client::AiClient::new(gemini_key);

        {
            let db = state.database.lock().await;
            if let Err(e) = db.add_ai_chat_message("user", &input.content) {
                return Json(
                    serde_json::json!({ "status": "error", "message": format!("Failed to save input: {}", e) }),
                );
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
                {
                    let db = state.database.lock().await;
                    let _ = db.add_ai_chat_message("model", &response);
                }

                // For now, we only push workouts to Garmin in the primary generation run.
                // Any JSON provided in chat could be applied later.
                Json(serde_json::json!({ "status": "success", "message": "Responded" }))
            }
            Err(e) => Json(serde_json::json!({ "status": "error", "message": e.to_string() })),
        }
    } else {
        Json(serde_json::json!({ "status": "error", "message": "No API key" }))
    }
}

async fn get_progression(State(state): State<ApiState>) -> Json<Vec<ProgressionResponse>> {
    let db = state.database.lock().await;
    let history = db.get_progression_history_raw().unwrap_or_default();

    let mut response = Vec::new();
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
    } else {
        println!("Garmin fetch_data() failed in get_recovery");
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
        // Filter done activities
        response.done = data
            .activities
            .into_iter()
            .filter(|a| a.start_time.starts_with(&today_prefix))
            .collect();

        // Filter planned activities
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
