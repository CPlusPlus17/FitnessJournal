use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum WorkoutType {
    Run,
    Bike,
    Strength,
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct Workout {
    pub id: String,
    pub start_time: DateTime<Utc>,
    pub duration_minutes: f64,
    pub workout_type: WorkoutType,
    pub distance_km: Option<f64>,
    pub avg_heart_rate: Option<f64>,
    pub calories: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ActivitySummary {
    pub time: DateTime<Utc>,
    pub name: String,
    pub duration_minutes: f64,
    pub distance_km: f64,
    pub calories: f64,
    pub avg_hr: f64,
    pub max_hr: f64,
    pub sport: String, // e.g., "Generic" (often mapped to specific sports in Garmin)
    pub sub_sport: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrainingTarget {
    pub workout_type: WorkoutType,
    pub target_duration_minutes: f64,
    pub target_distance_km: Option<f64>,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrainingPlan {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,

    pub workouts: Vec<TrainingTarget>,
}

// --- Garmin Connect Detailed Models ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GarminResponse {
    pub activities: Vec<GarminActivity>,
    pub plans: Vec<GarminPlan>,
    #[serde(default)]
    pub user_profile: Option<GarminProfile>,
    #[serde(default)]
    pub max_metrics: Option<GarminMaxMetrics>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GarminProfile {
    pub weight: Option<f64>,
    pub height: Option<f64>,
    #[serde(rename = "birthDate")]
    pub birth_date: Option<String>,
    #[serde(rename = "vo2MaxRunning")]
    pub vo2_max_running: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GarminMaxMetrics {
    #[serde(rename = "vo2MaxPrecise")]
    pub vo2_max_precise: Option<f64>,
    #[serde(rename = "fitnessAge")]
    pub fitness_age: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GarminPlan {
    pub name: String,
    #[serde(rename = "endDate")]
    pub end_date: String,
    #[serde(rename = "type")]
    pub plan_type: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GarminActivity {
    pub id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub activity_type: String,
    #[serde(rename = "startTimeLocal")]
    pub start_time: String,
    pub distance: Option<f64>,
    pub duration: Option<f64>,
    #[serde(rename = "averageHR")]
    pub average_hr: Option<f64>,
    #[serde(rename = "maxHR")]
    pub max_hr: Option<f64>,
    pub sets: Option<GarminSetsData>, 
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum GarminSetsData {
    Details(GarminSetContainer),
    Empty(Vec<serde_json::Value>), 
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GarminSetContainer {
    #[serde(rename = "exerciseSets")]
    pub exercise_sets: Vec<GarminSet>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GarminSet {
    #[serde(rename = "setType")]
    pub set_type: String, 
    #[serde(rename = "repetitionCount")]
    pub repetition_count: Option<i32>,
    pub weight: Option<f64>,
    pub duration: Option<f64>,
    #[serde(default)]
    pub exercises: Vec<GarminExercise>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GarminExercise {
    pub category: String,
    pub name: String,
}
