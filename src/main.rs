mod ai_client;
mod api;
mod bot;
mod coaching;
mod db;
mod garmin_api;
mod garmin_client;
mod garmin_login;
mod models;
mod workout_builder;

use crate::coaching::Coach;
use crate::db::Database;
use crate::garmin_client::GarminClient;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    println!("Starting Fitness Coach...");

    let database = Arc::new(Mutex::new(
        Database::new().expect("Failed to initialize SQLite database"),
    ));

    // Initialize components
    let garmin_client = Arc::new(GarminClient::new(database.clone()));
    let coach = Arc::new(Coach::new());

    let args: Vec<String> = std::env::args().collect();
    let is_daemon = args.len() > 1 && args.contains(&"--daemon".to_string());
    let is_signal = args.len() > 1 && args.contains(&"--signal".to_string());
    let is_api = args.len() > 1 && args.contains(&"--api".to_string());

    if let Some(pos) = args.iter().position(|a| a == "--test-upload") {
        if let Some(file) = args.get(pos + 1) {
            println!("Testing workout upload with file: {}", file);
            let json_str = std::fs::read_to_string(file)?;
            let builder = crate::workout_builder::WorkoutBuilder::new();
            let parsed: serde_json::Value = serde_json::from_str(&json_str)?;
            let payload = builder.build_workout_payload(&parsed, false);
            match garmin_client
                .api
                .connectapi_post("/workout-service/workout", &payload)
                .await
            {
                Ok(res) => println!("Success! Workout ID: {:?}", res.get("workoutId")),
                Err(e) => println!("Failed to create workout: {}", e),
            }
            return Ok(());
        }
    }


    if args.contains(&"--delete-workouts".to_string()) {
        println!("Fetching workouts to delete...");
        match garmin_client.api.get_workouts().await {
            Ok(workouts) => {
                if let Some(arr) = workouts.as_array() {
                    let mut to_delete = Vec::new();
                    for w in arr {
                        if let Some(name) = w.get("workoutName").and_then(|n| n.as_str()) {
                            if name.starts_with("Strength")
                                || name.starts_with("Test")
                                || name.starts_with("Full Body")
                                || name.starts_with("Upper")
                                || name.starts_with("Lower")
                            {
                                if let Some(wid) = w.get("workoutId").and_then(|i| i.as_i64()) {
                                    to_delete.push((wid, name.to_string()));
                                }
                            }
                        }
                    }

                    println!("Found {} workouts to delete.", to_delete.len());
                    for (wid, name) in to_delete {
                        let endpoint = format!("/workout-service/workout/{}", wid);
                        match garmin_client.api.connectapi_delete(&endpoint).await {
                            Ok(_) => println!("Deleted {} ({})", wid, name),
                            Err(e) => println!("Failed to delete {}: {}", wid, e),
                        }
                    }
                }
            }
            Err(e) => println!("Failed to fetch workouts: {}", e),
        }
        return Ok(());
    }

    if args.contains(&"--test-refresh".to_string()) {
        println!("Testing OAuth2 Token Refresh...");
        let temp_db = Arc::new(Mutex::new(
            Database::new().expect("Failed to initialize SQLite database"),
        ));
        let garmin_client = crate::garmin_client::GarminClient::new(temp_db);
        match garmin_client.api.refresh_oauth2().await {
            Ok(_) => println!("Successfully refreshed token!"),
            Err(e) => println!("Failed to refresh: {}", e),
        }
        return Ok(());
    }

    if args.contains(&"--login".to_string()) {
        use std::io::{self, Write};

        print!("Garmin Email: ");
        io::stdout().flush()?;
        let mut email = String::new();
        io::stdin().read_line(&mut email)?;
        let email = email.trim();

        let password = rpassword::prompt_password("Garmin Password: ")?;

        println!("Logging into Garmin Connect...");
        match crate::garmin_login::login_step_1(email, &password).await {
            Ok(crate::garmin_login::LoginResult::Success(o1, o2)) => {
                println!("Login successful!");
                std::fs::write(
                    "secrets/oauth1_token.json",
                    serde_json::to_string_pretty(&o1)?,
                )?;
                std::fs::write(
                    "secrets/oauth2_token.json",
                    serde_json::to_string_pretty(&o2)?,
                )?;
                println!(
                    "Saved credentials to secrets/oauth1_token.json and secrets/oauth2_token.json"
                );
            }
            Ok(crate::garmin_login::LoginResult::MfaRequired(session)) => {
                print!("Garmin MFA Code (Enter to submit): ");
                io::stdout().flush()?;
                let mut mfa_code = String::new();
                io::stdin().read_line(&mut mfa_code)?;
                let mfa_code = mfa_code.trim();

                println!("Submitting MFA code...");
                match crate::garmin_login::login_step_2_mfa(session, mfa_code).await {
                    Ok((o1, o2)) => {
                        println!("MFA successful!");
                        std::fs::write(
                            "secrets/oauth1_token.json",
                            serde_json::to_string_pretty(&o1)?,
                        )?;
                        std::fs::write(
                            "secrets/oauth2_token.json",
                            serde_json::to_string_pretty(&o2)?,
                        )?;
                        println!("Saved credentials to secrets/oauth1_token.json and secrets/oauth2_token.json");
                    }
                    Err(e) => println!("MFA login failed: {}", e),
                }
            }
            Err(e) => println!("Login failed: {}", e),
        }
        return Ok(());
    }

    if is_api {
        println!("Starting Fitness Coach in API mode.");
        if let Err(e) =
            api::run_server(database.clone(), garmin_client.clone(), coach.clone()).await
        {
            eprintln!("API Server crashed: {}", e);
        }
        return Ok(());
    }

    if is_signal {
        let bot = bot::BotController::new(garmin_client.clone(), coach.clone(), database.clone());
        bot.run().await;
        return Ok(());
    }

    if is_daemon {
        println!("Starting Fitness Coach in DAEMON mode. Will run every 24 hours.");
        loop {
            run_coach_pipeline(garmin_client.clone(), coach.clone(), database.clone()).await?;
            println!("Sleeping for 24 hours... zzz");
            tokio::time::sleep(tokio::time::Duration::from_secs(86400)).await;
        }
    } else {
        run_coach_pipeline(garmin_client.clone(), coach.clone(), database.clone()).await?;
    }

    Ok(())
}

pub async fn run_coach_pipeline(
    garmin_client: Arc<GarminClient>,
    coach: Arc<Coach>,
    database: Arc<Mutex<Database>>,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Fetch Detailed Data from Garmin Connect (Native Rust)
    println!("\nFetching detailed stats from Garmin Connect...");
    let (
        detailed_activities,
        active_plans,
        user_profile,
        max_metrics,
        scheduled_workouts,
        recovery,
    ) = match garmin_client.fetch_data().await {
        Ok(response) => {
            println!(
                "Found {} detailed activities, {} active plans, and {} scheduled workouts.",
                response.activities.len(),
                response.plans.len(),
                response.scheduled_workouts.len()
            );
            (
                response.activities,
                response.plans,
                response.user_profile,
                response.max_metrics,
                response.scheduled_workouts,
                response.recovery_metrics,
            )
        }
        Err(e) => {
            eprintln!("Failed to fetch detailed Garmin data: {}", e);
            (Vec::new(), Vec::new(), None, None, Vec::new(), None)
        }
    };

    // --- 2b. Sync Garmin Strength Sets to Local Database ---
    for act in &detailed_activities {
        if let Err(e) = database.lock().await.insert_activity(act) {
            eprintln!(
                "Failed to insert activity {} into DB: {}",
                act.id.unwrap_or(0),
                e
            );
        }
    }

    // Fetch 1RM/3RM/10RM History
    let progression_history = database
        .lock()
        .await
        .get_progression_history()
        .unwrap_or_default();
    println!(
        "Loaded progression history for {} exercises.",
        progression_history.len()
    );

    // 3. Define Context & Goals
    let mut context = crate::coaching::CoachContext {
        goals: vec![
            "Improve Marathon Time (Sub 4h)".to_string(),
            "Maintain Upper Body Strength (Hypertrophy)".to_string(),
            "Increase VO2Max".to_string(),
        ],
        constraints: vec![],
        available_equipment: vec![],
    };

    // Load active profile from profiles.json
    if let Ok(profile_data) = std::fs::read_to_string("profiles.json") {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&profile_data) {
            if let Some(active_name) = json.get("active_profile").and_then(|v| v.as_str()) {
                println!("Loaded active equipment profile: {}", active_name);
                if let Some(profile) = json.get("profiles").and_then(|p| p.get(active_name)) {
                    if let Some(constraints) = profile.get("constraints").and_then(|c| c.as_array())
                    {
                        context.constraints = constraints
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                    }
                    if let Some(equipment) = profile
                        .get("available_equipment")
                        .and_then(|e| e.as_array())
                    {
                        context.available_equipment = equipment
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                    }
                }
            }
        }
    }

    // Generate Brief
    println!("\nGenerating Coach Brief...");
    let brief = coach.generate_brief(
        &detailed_activities,
        &active_plans,
        &user_profile,
        &max_metrics,
        &scheduled_workouts,
        &recovery,
        &context,
        &progression_history,
    );

    println!("===================================================");
    println!("{}", brief);
    println!("===================================================");

    if let Ok(gemini_key) = std::env::var("GEMINI_API_KEY") {
        println!("\nGEMINI_API_KEY found! Generating workout via Gemini 3.1 Pro Preview...");

        // Initialize AI Client
        let ai_client = crate::ai_client::AiClient::new(gemini_key);

        println!("Cleaning up previously generated workouts before generating a new plan...");
        if let Err(e) = garmin_client.cleanup_ai_workouts().await {
            println!("Warning: failed to cleanup old AI workouts: {}", e);
        }

        println!("Wiping previous chat context...");
        if let Err(e) = database.lock().await.clear_ai_chat() {
            println!("Warning: failed to clear AI chat log: {}", e);
        }

        if let Err(e) = database.lock().await.add_ai_chat_message("user", &brief) {
            println!("Warning: failed to insert brief to AI chat log: {}", e);
        }

        match ai_client.generate_workout(&brief).await {
            Ok(markdown_response) => {
                println!("Received response from AI!");

                if let Err(e) = database
                    .lock()
                    .await
                    .add_ai_chat_message("model", &markdown_response)
                {
                    println!(
                        "Warning: failed to insert AI response to AI chat log: {}",
                        e
                    );
                }

                match crate::ai_client::AiClient::extract_json_block(&markdown_response) {
                    Ok(json_str) => {
                        let out_file = "generated_workouts.json";
                        std::fs::write(out_file, &json_str)?;
                        println!("Saved structured workout to {}", out_file);

                        // Upload to Garmin
                        println!("Uploading to Garmin Connect...");
                        let builder = crate::workout_builder::WorkoutBuilder::new();
                        let parsed: serde_json::Value =
                            serde_json::from_str(&json_str).unwrap_or(serde_json::Value::Null);

                        let workouts = if let Some(arr) = parsed.as_array() {
                            arr.clone()
                        } else {
                            vec![parsed]
                        };

                        for w in workouts {
                            if let Some(name) = w.get("workoutName").and_then(|n| n.as_str()) {
                                println!("Creating workout: {}...", name);
                            }

                            let mut payload = builder.build_workout_payload(&w, false);
                            let mut workout_id = None;

                            // Trying normal payload
                            match garmin_client
                                .api
                                .connectapi_post("/workout-service/workout", &payload)
                                .await
                            {
                                Ok(res) => {
                                    if let Some(id) = res.get("workoutId").and_then(|i| i.as_i64())
                                    {
                                        workout_id = Some(id);
                                        println!("Success! Workout ID: {}", id);
                                    }
                                }
                                Err(e) => {
                                    if e.to_string().contains("400") {
                                        println!("Failed with CSV mapping (400). Retrying with generic fallback...");
                                        payload = builder.build_workout_payload(&w, true);
                                        match garmin_client
                                            .api
                                            .connectapi_post("/workout-service/workout", &payload)
                                            .await
                                        {
                                            Ok(res) => {
                                                if let Some(id) =
                                                    res.get("workoutId").and_then(|i| i.as_i64())
                                                {
                                                    workout_id = Some(id);
                                                    println!(
                                                        "Success! (Generic Mode) Workout ID: {}",
                                                        id
                                                    );
                                                }
                                            }
                                            Err(e2) => println!(
                                                "Failed to create workout (even generic): {}",
                                                e2
                                            ),
                                        }
                                    } else {
                                        println!("Failed to create workout: {}", e);
                                    }
                                }
                            }

                            if let (Some(id), Some(sch_date)) =
                                (workout_id, w.get("scheduledDate").and_then(|d| d.as_str()))
                            {
                                println!("Scheduling workout {} on {}...", id, sch_date);
                                let sched_payload = serde_json::json!({ "date": sch_date });
                                let sched_endpoint = format!("/workout-service/schedule/{}", id);
                                match garmin_client
                                    .api
                                    .connectapi_post(&sched_endpoint, &sched_payload)
                                    .await
                                {
                                    Ok(_) => println!("Successfully scheduled on {}", sch_date),
                                    Err(e) => println!("Failed to schedule: {}", e),
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Could not extract JSON from AI response: {}", e);
                        println!("Raw Response:\n{}", markdown_response);
                    }
                }
            }
            Err(e) => eprintln!("Failed to call Gemini: {}", e),
        }
    } else {
        println!("\nNo GEMINI_API_KEY set. Skipping automatic workout generation.");
    }

    Ok(())
}
