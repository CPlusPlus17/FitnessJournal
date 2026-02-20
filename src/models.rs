use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
    #[serde(default)]
    pub scheduled_workouts: Vec<ScheduledWorkout>,
    #[serde(default)]
    pub recovery_metrics: Option<GarminRecoveryMetrics>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GarminRecoveryMetrics {
    pub sleep_score: Option<i32>,
    #[serde(default)]
    pub recent_sleep_scores: Vec<SleepScore>,
    pub current_body_battery: Option<i32>,
    pub training_readiness: Option<i32>,
    pub hrv_status: Option<String>,
    pub hrv_weekly_avg: Option<i32>,
    pub hrv_last_night_avg: Option<i32>,
    #[serde(default)]
    pub rhr_trend: Vec<i32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SleepScore {
    pub date: String,
    pub score: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScheduledWorkout {
    pub title: Option<String>,
    pub date: String,
    #[serde(default, alias = "sportTypeKey")]
    pub sport: Option<String>,
    #[serde(default, alias = "itemType", rename = "type")]
    pub item_type: Option<String>,
    #[serde(default, alias = "isRace")]
    pub is_race: Option<bool>,
    #[serde(default, alias = "primaryEvent")]
    pub primary_event: Option<bool>,
    pub duration: Option<f64>,
    pub distance: Option<f64>,
    pub description: Option<String>,
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
    #[serde(alias = "activityId")]
    pub id: Option<i64>,
    #[serde(alias = "activityName")]
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub activity_type: Option<String>,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExerciseMuscleMap {
    pub name: String,
    pub muscles: Vec<String>,
    pub frequency: i32,
}
