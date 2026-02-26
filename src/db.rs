use crate::models::GarminActivity;
use rusqlite::{params, Connection, Result};

const MAX_CHAT_HISTORY: i64 = 200;
const MAX_CHAT_MESSAGE_LEN: usize = 65_536;

pub type TrendHistoryItem = (f64, i32, String);
pub type ProgressionHistoryEntry = (String, f64, i32, String, Vec<TrendHistoryItem>);

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new() -> Result<Self> {
        // Use the env variable or default to the Docker environment path
        let db_path = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:///app/fitness_journal.db".to_string());

        let conn = Connection::open(db_path.replace("sqlite://", ""))?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS exercise_history (
                id INTEGER PRIMARY KEY,
                activity_id INTEGER NOT NULL,
                date TEXT NOT NULL,
                exercise_name TEXT NOT NULL,
                weight REAL NOT NULL,
                reps INTEGER NOT NULL,
                set_index INTEGER NOT NULL,
                UNIQUE(activity_id, set_index)
            )",
            (),
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS nutrition_logs (
                id INTEGER PRIMARY KEY,
                date TEXT UNIQUE NOT NULL,
                kcal INTEGER NOT NULL,
                protein_g INTEGER NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS kv_store (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS ai_chats (
                id INTEGER PRIMARY KEY,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at INTEGER NOT NULL
            )",
            [],
        )?;

        Ok(Database { conn })
    }

    pub fn log_nutrition(&self, date: &str, kcal: i32, protein_g: i32) -> Result<()> {
        self.conn.execute(
            "INSERT INTO nutrition_logs (date, kcal, protein_g) 
             VALUES (?1, ?2, ?3)
             ON CONFLICT(date) DO UPDATE SET 
             kcal = excluded.kcal, 
             protein_g = excluded.protein_g",
            params![date, kcal, protein_g],
        )?;
        Ok(())
    }

    pub fn get_latest_nutrition(&self) -> Result<Option<(String, i32, i32)>> {
        let mut stmt = self.conn.prepare(
            "SELECT date, kcal, protein_g FROM nutrition_logs ORDER BY date DESC LIMIT 1",
        )?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let date: String = row.get(0)?;
            let kcal: i32 = row.get(1)?;
            let protein_g: i32 = row.get(2)?;
            return Ok(Some((date, kcal, protein_g)));
        }
        Ok(None)
    }

    pub fn clear_ai_chat(&self) -> Result<()> {
        self.conn.execute("DELETE FROM ai_chats", [])?;
        Ok(())
    }

    pub fn add_ai_chat_message(&self, role: &str, content: &str) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_default();
        let safe_content: String = content.chars().take(MAX_CHAT_MESSAGE_LEN).collect();
        self.conn.execute(
            "INSERT INTO ai_chats (role, content, created_at) VALUES (?1, ?2, ?3)",
            params![role, safe_content, now],
        )?;
        self.conn.execute(
            "DELETE FROM ai_chats 
             WHERE id NOT IN (
                SELECT id FROM ai_chats ORDER BY id DESC LIMIT ?1
             )",
            params![MAX_CHAT_HISTORY],
        )?;
        Ok(())
    }

    pub fn get_ai_chat_history(&self) -> Result<Vec<(String, String, u64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT role, content, created_at FROM (
                SELECT id, role, content, created_at
                FROM ai_chats
                ORDER BY id DESC
                LIMIT ?1
             )
             ORDER BY id ASC",
        )?;
        let mut rows = stmt.query(params![MAX_CHAT_HISTORY])?;
        let mut history = Vec::new();

        while let Some(row) = rows.next()? {
            let role: String = row.get(0)?;
            let content: String = row.get(1)?;
            let created_at: u64 = row.get(2)?;
            history.push((role, content, created_at));
        }

        Ok(history)
    }

    pub fn insert_activity(&self, activity: &GarminActivity) -> Result<()> {
        if let Some(crate::models::GarminSetsData::Details(data)) = &activity.sets {
            let mut stmt = self.conn.prepare(
                "INSERT OR IGNORE INTO exercise_history 
                (activity_id, date, exercise_name, weight, reps, set_index) 
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;

            for (index, set) in data.exercise_sets.iter().enumerate() {
                // We only care about active working sets
                if set.set_type != "ACTIVE" {
                    continue;
                }

                if let Some(exercise) = set.exercises.first() {
                    let weight = set.weight.unwrap_or(0.0) / 1000.0; // convert g to kg
                    let reps = set.repetition_count.unwrap_or(0);

                    if reps > 0 {
                        stmt.execute((
                            activity.id,
                            &activity.start_time,
                            &exercise.category, // e.g. "BENCH_PRESS"
                            weight,
                            reps,
                            index as i32,
                        ))?;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn get_progression_history(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT exercise_name, weight, reps, date
             FROM (
                SELECT
                    exercise_name,
                    weight,
                    reps,
                    date,
                    ROW_NUMBER() OVER (
                        PARTITION BY exercise_name
                        ORDER BY weight DESC, reps DESC, date DESC
                    ) AS row_rank
                FROM exercise_history
             )
             WHERE row_rank = 1
             ORDER BY exercise_name ASC",
        )?;

        let mut rows = stmt.query(())?;
        let mut history = Vec::new();

        while let Some(row) = rows.next()? {
            let name: String = row.get(0)?;
            let weight: f64 = row.get(1)?;
            let reps: i32 = row.get(2)?;
            let date: String = row.get(3)?;

            history.push(format!(
                "- **{}**: {}kg x {} ({})",
                name, weight, reps, date
            ));
        }

        Ok(history)
    }

    pub fn get_progression_history_raw(&self) -> Result<Vec<ProgressionHistoryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT exercise_name, weight, reps, date
             FROM (
                SELECT
                    exercise_name,
                    weight,
                    reps,
                    date,
                    ROW_NUMBER() OVER (
                        PARTITION BY exercise_name, date
                        ORDER BY weight DESC, reps DESC, set_index DESC
                    ) AS row_rank
                FROM exercise_history
             )
             WHERE row_rank = 1
             ORDER BY exercise_name ASC, date ASC",
        )?;

        let mut rows = stmt.query(())?;
        use std::collections::BTreeMap;
        let mut history_map: BTreeMap<String, Vec<TrendHistoryItem>> = BTreeMap::new();

        while let Some(row) = rows.next()? {
            let name: String = row.get(0)?;
            let weight: f64 = row.get(1)?;
            let reps: i32 = row.get(2)?;
            let date: String = row.get(3)?;

            history_map
                .entry(name)
                .or_default()
                .push((weight, reps, date));
        }

        let mut result = Vec::new();
        for (name, history) in history_map {
            let mut max_weight = 0.0;
            let mut best_reps = 0;
            let mut best_date = String::new();
            for (weight, reps, date) in &history {
                if *weight > max_weight
                    || ((*weight - max_weight).abs() < f64::EPSILON && *reps > best_reps)
                {
                    max_weight = *weight;
                    best_reps = *reps;
                    best_date = date.clone();
                }
            }
            result.push((name, max_weight, best_reps, best_date, history));
        }

        Ok(result)
    }

    pub fn get_recent_muscle_heatmap(
        &self,
        days: u32,
    ) -> Result<Vec<crate::models::ExerciseMuscleMap>> {
        // Find active sets in the last N days
        // We'll calculate the cutoff date in the API or DB level using sqlite date modifiers
        // We group by exercise_name.
        let mut stmt = self.conn.prepare(
            "SELECT exercise_name, COUNT(*) as frequency 
             FROM exercise_history 
             WHERE date >= date('now', ?1)
             GROUP BY exercise_name",
        )?;

        // e.g. "-14 days"
        let modifier = format!("-{} days", days);
        let mut rows = stmt.query(params![modifier])?;

        let mut heatmap = Vec::new();

        while let Some(row) = rows.next()? {
            let name: String = row.get(0)?;
            let frequency: i32 = row.get(1)?;

            // Map the exercise category back to react-body-highlighter muscles
            let muscles = match name.as_str() {
                "BENCH_PRESS" | "PUSH_UP" => vec![
                    "chest".to_string(),
                    "triceps".to_string(),
                    "front-deltoids".to_string(),
                ],
                "ROW" => vec![
                    "upper-back".to_string(),
                    "lower-back".to_string(),
                    "biceps".to_string(),
                    "back-deltoids".to_string(),
                ],
                "PULL_UP" | "PULL_DOWN" => vec![
                    "upper-back".to_string(),
                    "biceps".to_string(),
                    "back-deltoids".to_string(),
                ],
                "SQUAT" | "LUNGE" => vec![
                    "quadriceps".to_string(),
                    "gluteal".to_string(),
                    "hamstring".to_string(),
                    "calves".to_string(),
                ],
                "DEADLIFT" => vec![
                    "hamstring".to_string(),
                    "gluteal".to_string(),
                    "lower-back".to_string(),
                    "forearm".to_string(),
                    "trapezius".to_string(),
                ],
                "CALF_RAISE" => vec!["calves".to_string()],
                "SHOULDER_PRESS" | "FRONT_RAISE" | "LATERAL_RAISE" => vec![
                    "front-deltoids".to_string(),
                    "back-deltoids".to_string(),
                    "triceps".to_string(),
                ],
                "TRICEPS_EXTENSION" => vec!["triceps".to_string()],
                "BICEP_CURL" => vec!["biceps".to_string()],
                "CORE" | "PLANK" | "SIT_UP" => vec!["abs".to_string(), "obliques".to_string()],
                _ => vec![],
            };

            if !muscles.is_empty() {
                heatmap.push(crate::models::ExerciseMuscleMap {
                    name,
                    muscles,
                    frequency,
                });
            }
        }

        Ok(heatmap)
    }

    pub fn get_garmin_cache(&self) -> Result<Option<(String, u64)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT value, updated_at FROM kv_store WHERE key = 'garmin_cache'")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let value: String = row.get(0)?;
            let updated_at: u64 = row.get(1)?;
            return Ok(Some((value, updated_at)));
        }
        Ok(None)
    }

    pub fn set_garmin_cache(&self, value: &str) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_default();
        self.conn.execute(
            "INSERT INTO kv_store (key, value, updated_at) 
             VALUES ('garmin_cache', ?1, ?2)
             ON CONFLICT(key) DO UPDATE SET 
             value = excluded.value, 
             updated_at = excluded.updated_at",
            params![value, now],
        )?;
        Ok(())
    }

    pub fn clear_garmin_cache(&self) -> Result<()> {
        self.conn.execute("DELETE FROM kv_store WHERE key = 'garmin_cache'", [])?;
        Ok(())
    }
}
