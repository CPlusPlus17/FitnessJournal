use lazy_static::lazy_static;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashMap;
use strsim::levenshtein;

// ... (constants remain the same, so we will keep them as is and just replace the struct and below)

// Constants mapping ported from Python
const SPORT_TYPE_STRENGTH: &str = "strength_training";
const SPORT_TYPE_ID_STRENGTH: i32 = 5;

const STEP_TYPE_WARMUP: &str = "warmup";
const STEP_TYPE_ID_WARMUP: i32 = 1;

const STEP_TYPE_COOLDOWN: &str = "cooldown";
const STEP_TYPE_ID_COOLDOWN: i32 = 2;

const STEP_TYPE_INTERVAL: &str = "interval";
const STEP_TYPE_ID_INTERVAL: i32 = 3;

const STEP_TYPE_REST: &str = "rest";
const STEP_TYPE_ID_REST: i32 = 5;

const CONDITION_REPS: &str = "reps";
const CONDITION_ID_REPS: i32 = 10;

const CONDITION_TIME: &str = "time";
const CONDITION_ID_TIME: i32 = 2;

const CONDITION_LAP_BUTTON: &str = "lap.button";
const CONDITION_ID_LAP_BUTTON: i32 = 1;

const TARGET_NO_TARGET: &str = "no.target";
const TARGET_ID_NO_TARGET: i32 = 1;

const UNIT_KILOGRAM: &str = "kilogram";
const UNIT_ID_KILOGRAM: i32 = 8;

lazy_static! {
    static ref MANUAL_OVERRIDES: HashMap<&'static str, (&'static str, &'static str)> = {
        let mut m = HashMap::new();
        m.insert("BENT_OVER_ROW", ("ROW", "BARBELL_ROW"));
        m.insert(
            "TRICEPS_EXTENSION",
            ("TRICEPS_EXTENSION", "TRICEP_EXTENSION"),
        );
        m.insert("PULL_UP", ("PULL_UP", "CHIN_UP"));
        m.insert("PUSH_UP", ("PUSH_UP", "PUSH_UP"));
        m.insert("LUNGE", ("LUNGE", "LUNGE"));
        m.insert("SQUAT", ("SQUAT", "SQUAT"));
        m.insert("DEADLIFT", ("DEADLIFT", "DEADLIFT"));
        m.insert("BENCH_PRESS", ("BENCH_PRESS", "BENCH_PRESS"));
        m.insert("YOGA", ("WARM_UP", "STRETCH_SIDE"));
        m.insert("STRETCHING", ("WARM_UP", "STRETCH_SIDE"));
        m.insert("OVERHEAD_PRESS", ("SHOULDER_PRESS", "SHOULDER_PRESS"));
        m.insert("SHOULDER_PRESS", ("SHOULDER_PRESS", "SHOULDER_PRESS"));
        m.insert("PLANK", ("PLANK", "PLANK"));
        m.insert("LAT_PULLDOWN", ("PULL_UP", "CLOSE_GRIP_LAT_PULLDOWN"));
        m.insert("RUSSIAN_TWIST", ("CORE", "RUSSIAN_TWIST"));
        m.insert("DUMBBELL_ROW", ("ROW", "BENT_OVER_ROW_WITH_DUMBELL"));
        m.insert(
            "BICEP_CURL",
            ("CURL", "STANDING_ALTERNATING_DUMBBELL_CURLS"),
        );
        m.insert("LUNGES", ("LUNGE", "ALTERNATING_DUMBBELL_LUNGE"));
        m.insert("GOBLET_SQUAT", ("SQUAT", "GOBLET_SQUAT"));
        m.insert("FACE_PULL", ("ROW", "FACE_PULL"));
        m.insert("LATERAL_RAISE", ("LATERAL_RAISE", "LATERAL_RAISE"));
        m.insert("CALF_RAISE", ("CALF_RAISE", "CALF_RAISE"));
        m
    };
}

pub struct WorkoutBuilder {
    exercise_db: HashMap<String, (String, String)>,
}

impl WorkoutBuilder {
    pub fn new() -> Self {
        let mut builder = Self {
            exercise_db: HashMap::new(),
        };
        builder.load_exercise_db("Garmin Exercises Database - Exercises.csv");
        builder
    }

    fn load_exercise_db(&mut self, path: &str) {
        if !std::path::Path::new(path).exists() {
            println!(
                "Warning: Exercise DB CSV not found at {}. Using name as key.",
                path
            );
            return;
        }

        if let Ok(mut rdr) = csv::ReaderBuilder::new()
            .flexible(true)
            .has_headers(false) // Handle them manually
            .from_path(path)
        {
            let mut records = rdr.records();
            if let Some(_) = records.next() {} // Skip Group Headers

            // Read actual headers to find indexes
            let mut name_idx = 0;
            let mut cat_idx = 0;
            let mut id_idx = 0;

            if let Some(Ok(headers)) = records.next() {
                for (i, v) in headers.iter().enumerate() {
                    let h = v.to_uppercase();
                    if h == "NAME" {
                        name_idx = i;
                    }
                    if h == "CATEGORY_GARMIN" {
                        cat_idx = i;
                    }
                    if h == "NAME_GARMIN" {
                        id_idx = i;
                    }
                }
            }

            for result in records {
                if let Ok(row) = result {
                    if let (Some(name), Some(cat), Some(id)) =
                        (row.get(name_idx), row.get(cat_idx), row.get(id_idx))
                    {
                        let human_name = name.trim().to_uppercase();
                        let cat_key = cat.trim().to_string();
                        let ex_key = id.trim().to_string();

                        if !human_name.is_empty() && !cat_key.is_empty() && !ex_key.is_empty() {
                            let val = (cat_key.clone(), ex_key.clone());

                            self.exercise_db.insert(human_name.clone(), val.clone());
                            self.exercise_db.insert(ex_key.clone(), val.clone());
                            self.exercise_db
                                .insert(human_name.replace(" ", "_"), val.clone());
                            self.exercise_db
                                .insert(ex_key.replace("_", " "), val.clone());
                            self.exercise_db.insert(
                                human_name
                                    .replace("-", "")
                                    .replace(" ", "")
                                    .replace("_", ""),
                                val.clone(),
                            );
                            self.exercise_db
                                .insert(ex_key.replace("_", "").replace("-", ""), val);
                        }
                    }
                }
            }
        } else {
            println!("Warning: Could not read CSV at {}", path);
        }
    }

    pub fn resolve_exercise(&self, name: &str) -> (Option<String>, Option<String>) {
        let clean = name.trim().to_uppercase();

        if let Some((cat, ex)) = MANUAL_OVERRIDES.get(clean.as_str()) {
            return (Some(cat.to_string()), Some(ex.to_string()));
        }

        if let Some(val) = self.exercise_db.get(&clean) {
            return (Some(val.0.clone()), Some(val.1.clone()));
        }

        let norm_input = clean.replace("_", "").replace(" ", "").replace("-", "");
        if let Some(val) = self.exercise_db.get(&norm_input) {
            return (Some(val.0.clone()), Some(val.1.clone()));
        }

        if clean.contains("_") {
            return (Some(clean.clone()), Some(clean));
        }

        // Fuzzy fallback
        let mut best_match: Option<String> = None;
        let mut best_distance = usize::MAX;

        for key in self.exercise_db.keys() {
            let distance = levenshtein(&clean, key);
            // Threshold for acceptable match (e.g. within 3 edits)
            if distance < best_distance && distance <= 3 {
                best_distance = distance;
                best_match = Some(key.clone());
            }
        }

        if let Some(best_key) = best_match {
            if let Some(val) = self.exercise_db.get(&best_key) {
                // If fuzzy match differs from exact clean input, log it for debugging
                println!(
                    "Fuzzy match: '{}' -> '{}' (distance: {})",
                    name, best_key, best_distance
                );
                return (Some(val.0.clone()), Some(val.1.clone()));
            }
        }

        (None, None)
    }

    pub fn parse_duration(val: &Value) -> Option<i64> {
        match val {
            Value::Number(n) => n.as_i64(),
            Value::String(s) => {
                lazy_static! {
                    static ref RE: Regex = Regex::new(r"\d+").unwrap();
                }
                if let Some(caps) = RE.captures(s) {
                    if let Ok(parsed) = caps[0].parse::<i64>() {
                        if s.to_lowercase().contains("min") {
                            return Some(parsed * 60);
                        }
                        return Some(parsed);
                    }
                }
                None
            }
            _ => None,
        }
    }

    pub fn parse_weight(val: &Value) -> Option<f64> {
        match val {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => {
                lazy_static! {
                    static ref RE: Regex = Regex::new(r"[\d\.]+").unwrap();
                }
                if let Some(caps) = RE.captures(s) {
                    return caps[0].parse::<f64>().ok();
                }
                None
            }
            _ => None,
        }
    }

    pub fn build_workout_payload(&self, data: &Value, robust: bool) -> Value {
        let mut steps_payload = Vec::new();
        let mut order = 1;

        if let Some(steps) = data.get("steps").and_then(|s| s.as_array()) {
            for step in steps {
                let raw_name = step
                    .get("exercise")
                    .and_then(|e| e.as_str())
                    .unwrap_or("BENCH_PRESS");
                let phase = step
                    .get("phase")
                    .and_then(|p| p.as_str())
                    .unwrap_or("interval")
                    .to_lowercase();

                let step_type_id = if phase == "warmup" || phase == "warm_up" {
                    STEP_TYPE_ID_WARMUP
                } else if phase == "cooldown" || phase == "cool_down" || phase == "stretching" {
                    STEP_TYPE_ID_COOLDOWN
                } else {
                    STEP_TYPE_ID_INTERVAL
                };

                let step_type_key = if step_type_id == STEP_TYPE_ID_WARMUP {
                    STEP_TYPE_WARMUP
                } else if step_type_id == STEP_TYPE_ID_COOLDOWN {
                    STEP_TYPE_COOLDOWN
                } else {
                    STEP_TYPE_INTERVAL
                };

                let (mut cat_key, mut ex_key) = self.resolve_exercise(raw_name);

                if cat_key.is_none() {
                    let fallback = raw_name.to_uppercase().replace(" ", "_");
                    cat_key = Some(fallback.clone());
                    ex_key = Some(fallback);
                }

                let reps = step.get("reps");
                let duration = step.get("time").or_else(|| step.get("duration"));

                let mut end_cond_id = CONDITION_ID_LAP_BUTTON;
                let mut end_cond_key = CONDITION_LAP_BUTTON;
                let mut end_val: Option<Value> = None;

                if reps.is_some()
                    && step_type_id != STEP_TYPE_ID_WARMUP
                    && step_type_id != STEP_TYPE_ID_COOLDOWN
                {
                    if let Some(r_str) = reps.unwrap().as_str() {
                        if r_str.to_uppercase().contains("AMRAP") {
                            end_cond_id = CONDITION_ID_LAP_BUTTON;
                            end_cond_key = CONDITION_LAP_BUTTON;
                        } else if let Ok(n) = r_str.parse::<i64>() {
                            end_val = Some(json!(n));
                            end_cond_id = CONDITION_ID_REPS;
                            end_cond_key = CONDITION_REPS;
                        } else {
                            end_cond_id = CONDITION_ID_LAP_BUTTON;
                            end_cond_key = CONDITION_LAP_BUTTON;
                        }
                    } else if let Some(n) = reps.unwrap().as_i64() {
                        end_val = Some(json!(n));
                        end_cond_id = CONDITION_ID_REPS;
                        end_cond_key = CONDITION_REPS;
                    }
                } else if let Some(d) = duration {
                    if let Some(sec) = Self::parse_duration(d) {
                        end_cond_id = CONDITION_ID_TIME;
                        end_cond_key = CONDITION_TIME;
                        end_val = Some(json!(sec));
                    }
                }

                let weight_val = step.get("weight").and_then(|w| Self::parse_weight(w));

                let mut category = cat_key.clone();
                let mut exercise_name = ex_key.clone();
                let note = step.get("note").and_then(|n| n.as_str()).unwrap_or("");

                let mut description = if note.is_empty() {
                    None
                } else {
                    Some(note.to_string())
                };

                if robust {
                    category = None;
                    exercise_name = None;
                    let mut desc = format!(
                        "Exercise: {} ({}). {}",
                        raw_name,
                        ex_key.clone().unwrap_or_default(),
                        note
                    );
                    if let Some(w) = weight_val {
                        desc.push_str(&format!(" Target: {}kg", w));
                    }
                    description = Some(desc.trim().to_string());
                }

                let mut step_dict = json!({
                    "type": "ExecutableStepDTO",
                    "stepOrder": order,
                    "stepType": {
                        "stepTypeId": step_type_id,
                        "stepTypeKey": step_type_key,
                    },
                    "childStepId": null,
                    "description": description,
                    "endCondition": {
                        "conditionTypeId": end_cond_id,
                        "conditionTypeKey": end_cond_key,
                    },
                    "endConditionValue": end_val,
                    "targetType": {
                        "workoutTargetTypeId": TARGET_ID_NO_TARGET,
                        "workoutTargetTypeKey": TARGET_NO_TARGET,
                    },
                    "category": category,
                    "exerciseName": exercise_name,
                });

                if let Some(w) = weight_val {
                    if !robust {
                        step_dict
                            .as_object_mut()
                            .unwrap()
                            .insert("weightValue".to_string(), json!(w));
                        step_dict.as_object_mut().unwrap().insert(
                            "weightUnit".to_string(),
                            json!({
                                "unitId": UNIT_ID_KILOGRAM,
                                "unitKey": UNIT_KILOGRAM,
                                "factor": 1000.0
                            }),
                        );
                    }
                }

                steps_payload.push(step_dict);
                order += 1;

                if step_type_id == STEP_TYPE_ID_INTERVAL {
                    if let Some(rest) = step.get("rest") {
                        if let Some(rest_sec) = Self::parse_duration(rest) {
                            steps_payload.push(json!({
                                "type": "ExecutableStepDTO",
                                "stepOrder": order,
                                "stepType": {
                                    "stepTypeId": STEP_TYPE_ID_REST,
                                    "stepTypeKey": STEP_TYPE_REST,
                                },
                                "childStepId": null,
                                "endCondition": {
                                    "conditionTypeId": CONDITION_ID_TIME,
                                    "conditionTypeKey": CONDITION_TIME,
                                },
                                "endConditionValue": rest_sec,
                                "targetType": {
                                    "workoutTargetTypeId": TARGET_ID_NO_TARGET,
                                    "workoutTargetTypeKey": TARGET_NO_TARGET,
                                }
                            }));
                            order += 1;
                        }
                    }
                }
            }
        }

        let workout_name = data
            .get("workoutName")
            .and_then(|n| n.as_str())
            .unwrap_or("Imported Strength Workout");
        let description = data.get("description").and_then(|d| d.as_str());

        json!({
            "workoutName": workout_name,
            "description": description,
            "sportType": {
                "sportTypeId": SPORT_TYPE_ID_STRENGTH,
                "sportTypeKey": SPORT_TYPE_STRENGTH,
            },
            "workoutSegments": [
                {
                    "segmentOrder": 1,
                    "sportType": {
                        "sportTypeId": SPORT_TYPE_ID_STRENGTH,
                        "sportTypeKey": SPORT_TYPE_STRENGTH,
                    },
                    "workoutSteps": steps_payload
                }
            ]
        })
    }
}
