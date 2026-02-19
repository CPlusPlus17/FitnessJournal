use std::sync::Arc;
use tokio::sync::Mutex;
use axum::{
    routing::get,
    Router,
    Json,
    extract::State,
};
use tower_http::cors::{Any, CorsLayer};
use std::net::SocketAddr;

use crate::grafana_client::GrafanaClient;
use crate::garmin_client::GarminClient;
use crate::coaching::Coach;
use crate::db::Database;
use serde::Serialize;

#[derive(Clone)]
pub struct ApiState {
    pub database: Arc<Mutex<Database>>,
    pub grafana_client: Arc<GrafanaClient>,
    pub garmin_client: Arc<GarminClient>,
    pub coach: Arc<Coach>,
}

#[derive(Serialize)]
pub struct ProgressionResponse {
    pub exercise_name: String,
    pub max_weight: f64,
    pub reps: i32,
    pub date: String,
}

#[derive(Serialize)]
pub struct RecoveryResponse {
    pub body_battery: Option<i32>,
    pub sleep_score: Option<i32>,
}

pub async fn run_server(
    database: Arc<Mutex<Database>>,
    grafana_client: Arc<GrafanaClient>,
    garmin_client: Arc<GarminClient>,
    coach: Arc<Coach>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let state = ApiState { database, grafana_client, garmin_client, coach };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/progression", get(get_progression))
        .route("/api/recovery", get(get_recovery))
        .route("/api/generate", axum::routing::post(trigger_generate))
        .route("/api/muscle_heatmap", get(get_muscle_heatmap))
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
        state.grafana_client.clone(),
        state.garmin_client.clone(),
        state.coach.clone(),
        state.database.clone(),
    ).await {
        Ok(_) => Json(serde_json::json!({ "status": "success", "message": "Workouts generated and pushed to Garmin" })),
        Err(e) => Json(serde_json::json!({ "status": "error", "message": e.to_string() })),
    }
}

async fn get_progression(State(state): State<ApiState>) -> Json<Vec<ProgressionResponse>> {
    let db = state.database.lock().await;
    let history = db.get_progression_history_raw().unwrap_or_default();
    
    let mut response = Vec::new();
    for (name, weight, reps, date) in history {
        response.push(ProgressionResponse {
            exercise_name: name,
            max_weight: weight,
            reps,
            date, 
        });
    }

    Json(response)
}

async fn get_muscle_heatmap(State(state): State<ApiState>) -> Json<Vec<crate::models::ExerciseMuscleMap>> {
    let db = state.database.lock().await;
    let heatmap = db.get_recent_muscle_heatmap(14).unwrap_or_default();
    Json(heatmap)
}

async fn get_recovery(State(state): State<ApiState>) -> Json<RecoveryResponse> {
    let mut response = RecoveryResponse {
        body_battery: None,
        sleep_score: None,
    };

    if let Ok(data) = state.garmin_client.fetch_data() {
        if let Some(metrics) = data.recovery_metrics {
            response.body_battery = metrics.current_body_battery;
            response.sleep_score = metrics.sleep_score;
        }
    }

    Json(response)
}
