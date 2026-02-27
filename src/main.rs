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
use clap::Parser;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Parser, Debug)]
#[command(name = "fitness_journal", about = "Fitness Coach AI")]
struct Cli {
    #[arg(
        long,
        help = "Run as a background daemon calculating workloads every 24h"
    )]
    daemon: bool,
    #[arg(long, help = "Start Signal bot listener")]
    signal: bool,
    #[arg(long, help = "Start the web dashboard REST API")]
    api: bool,
    #[arg(long, help = "Login to Garmin Connect globally")]
    login: bool,
    #[arg(long, help = "Test uploading a local JSON file to Garmin")]
    test_upload: Option<String>,
    #[arg(long, help = "Test fetching and printing a specific workout ID")]
    test_fetch: Option<String>,
    #[arg(long, help = "Test fetching an arbitrary Garmin URL")]
    test_fetch_url: Option<String>,
    #[arg(long, help = "Delete ALL previously generated AI workouts in Garmin")]
    delete_workouts: bool,
    #[arg(long, help = "Test force-refreshing OAuth2 Garmin tokens")]
    test_refresh: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    println!("Starting Fitness Coach...");

    let database = match Database::new() {
        Ok(db) => Arc::new(Mutex::new(db)),
        Err(e) => {
            eprintln!("\n{}", "=".repeat(60));
            eprintln!("🛑 DATABASE INITIALIZATION ERROR 🛑");
            eprintln!("Failed to open or create the SQLite database.");
            eprintln!("Error details: {}", e);
            eprintln!("\n📝 Troubleshooting (Docker Users):");
            eprintln!("If you are using docker-compose, 'fitness_journal.db' might have been automatically created as a DIRECTORY instead of a file.");
            eprintln!("Please run these commands to fix this issue:");
            eprintln!("  1. docker-compose down");
            eprintln!("  2. rm -rf fitness_journal.db");
            eprintln!("  3. touch fitness_journal.db");
            eprintln!("  4. docker-compose up -d");
            eprintln!("{}\n", "=".repeat(60));
            std::process::exit(1);
        }
    };

    let coach = Arc::new(Coach::new());

    let args = Cli::parse();
    let is_daemon = args.daemon;
    let is_signal = args.signal;
    let is_api = args.api;

    if args.login {
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
                write_secret_json_file("secrets/oauth1_token.json", &o1)?;
                write_secret_json_file("secrets/oauth2_token.json", &o2)?;
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
                        write_secret_json_file("secrets/oauth1_token.json", &o1)?;
                        write_secret_json_file("secrets/oauth2_token.json", &o2)?;
                        println!("Saved credentials to secrets/oauth1_token.json and secrets/oauth2_token.json");
                    }
                    Err(e) => println!("MFA login failed: {}", e),
                }
            }
            Err(e) => println!("Login failed: {}", e),
        }
        return Ok(());
    }

    let garmin_client = Arc::new(GarminClient::new(database.clone()));

    if let Some(file) = args.test_upload {
        println!("Testing workout upload with file: {}", file);
        let json_str = std::fs::read_to_string(&file)?;
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
    }

    if let Some(workout_id) = args.test_fetch {
        println!("Fetching workout ID '{}' from Garmin...", workout_id);
        let endpoint = format!("/workout-service/workout/{}", workout_id);
        match garmin_client.api.connectapi_get(&endpoint).await {
            Ok(res) => println!("Response Payload:\n{}", serde_json::to_string_pretty(&res)?),
            Err(e) => println!("Failed: {}", e),
        }
        return Ok(());
    }

    if let Some(url) = args.test_fetch_url {
        println!("Fetching URL '{}' from Garmin...", url);
        match garmin_client.api.connectapi_get(&url).await {
            Ok(res) => println!("Response Payload:\n{}", serde_json::to_string_pretty(&res)?),
            Err(e) => println!("Failed: {}", e),
        }
        return Ok(());
    }

    if args.delete_workouts {
        println!("Fetching workouts to delete...");
        match garmin_client.api.get_workouts().await {
            Ok(workouts) => {
                if let Some(arr) = workouts.as_array() {
                    let mut to_delete = Vec::new();
                    for w in arr {
                        if let Some(name) = w.get("workoutName").and_then(|n| n.as_str()) {
                            if crate::garmin_client::is_ai_managed_workout(name) {
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

    if args.test_refresh {
        println!("Testing OAuth2 Token Refresh...");
        let temp_db = Arc::new(Mutex::new(
            Database::new().expect("Failed to initialize SQLite database"),
        ));
        let garmin_client_refresh = crate::garmin_client::GarminClient::new(temp_db);
        match garmin_client_refresh.api.refresh_oauth2().await {
            Ok(_) => println!("Successfully refreshed token!"),
            Err(e) => println!("Failed to refresh: {}", e),
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
        if is_daemon {
            tokio::spawn(async move {
                bot.run().await;
            });
        } else {
            bot.run().await;
            return Ok(());
        }
    }

    if is_daemon {
        println!("Starting Fitness Coach in DAEMON mode. Will run every 24 hours.");
        crate::bot::start_morning_notifier(garmin_client.clone());
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

    let mut context = crate::coaching::CoachContext {
        goals: vec![
            "Improve Marathon Time (Sub 4h)".to_string(),
            "Maintain Upper Body Strength (Hypertrophy)".to_string(),
            "Increase VO2Max".to_string(),
        ],
        constraints: vec![],
        available_equipment: vec![],
    };

    let mut auto_analyze_sports = Vec::new();

    // Load active profile from profiles.json
    if let Ok(profile_data) = std::fs::read_to_string("profiles.json") {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&profile_data) {
            if let Some(active_name) = json.get("active_profile").and_then(|v| v.as_str()) {
                println!("Loaded active equipment profile: {}", active_name);
                if let Some(profile) = json.get("profiles").and_then(|p| p.get(active_name)) {
                    if let Some(goals) = profile.get("goals").and_then(|g| g.as_array()) {
                        let parsed_goals: Vec<String> = goals
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                        if !parsed_goals.is_empty() {
                            context.goals = parsed_goals;
                        } else {
                            println!(
                                "Warning: profile '{}' has no valid goals. Falling back to default goals.",
                                active_name
                            );
                        }
                    }
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
                    if let Some(sports) = profile
                        .get("auto_analyze_sports")
                        .and_then(|s| s.as_array())
                    {
                        auto_analyze_sports = sports
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                    }
                }
            }
        }
    }

    // --- 2c. Auto-Analyze Activities (Signal Cheerleader) ---
    if let Ok(gemini_key) = std::env::var("GEMINI_API_KEY") {
        if !auto_analyze_sports.is_empty() {
            let ai_client = crate::ai_client::AiClient::new(gemini_key.clone());
            let db = database.lock().await;

            // Only analyze recent activities (from today or yesterday) to avoid spamming 50+ backlogs
            let today = chrono::Local::now();
            let yesterday = today - chrono::Duration::days(1);
            let today_str = today.format("%Y-%m-%d").to_string();
            let yesterday_str = yesterday.format("%Y-%m-%d").to_string();

            for act in &detailed_activities {
                if !act.start_time.starts_with(&today_str)
                    && !act.start_time.starts_with(&yesterday_str)
                {
                    continue;
                }

                if let (Some(id), Some(act_type)) = (act.id, act.get_activity_type()) {
                    if auto_analyze_sports.contains(&act_type.to_string()) {
                        let is_analyzed = db.is_activity_analyzed(id).unwrap_or(false);
                        if !is_analyzed {
                            println!("Activity {} ({}) matches auto_analyze_sports. Requesting analysis...", id, act_type);

                            let prompt = format!(
                                "Please provide an in-depth analysis of this completed fitness activity. Be encouraging but highly analytical.\n\nYou have been provided with the complete, raw JSON payload direct from Garmin. It contains many undocumented fields, extra metrics, recovery data, elevation, stress, cadence, temperatures, or detailed exercise sets.\n\nPlease actively hunt through this raw JSON and surface interesting insights, anomalies, or performance correlations that wouldn't be obvious from just the basic time/distance metrics. Explain what these deeper metrics mean for the athlete's progress.\n\nKeep the response concise enough for a messaging app (max 2-3 short paragraphs) and format it directly as text without any markdown wrappers.\n\nHere is the raw activity data:\n\n{}",
                                serde_json::to_string(act).unwrap_or_default()
                            );

                            match ai_client.generate_workout(&prompt).await {
                                Ok(analysis) => {
                                    println!("Analysis generated! Broadcasting via Signal...");
                                    let msg = format!(
                                        "📊 **Activity Analysis: {}**\n\n{}",
                                        act.name.as_deref().unwrap_or("Untitled Workout"),
                                        analysis
                                    );
                                    crate::bot::broadcast_message(&msg).await;

                                    if let Err(e) =
                                        db.save_activity_analysis(id, &act.start_time, &analysis)
                                    {
                                        eprintln!("Failed to save activity analysis to DB: {}", e);
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to generate analysis for {}: {}", id, e)
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Generate Brief
    println!("\nGenerating Coach Brief...");
    let brief = coach.generate_brief(crate::coaching::BriefInput {
        detailed_activities: &detailed_activities,
        plans: &active_plans,
        profile: &user_profile,
        metrics: &max_metrics,
        scheduled_workouts: &scheduled_workouts,
        recovery_metrics: &recovery,
        context: &context,
        progression_history: &progression_history,
    });

    println!("Coach brief generated ({} characters).", brief.len());
    if std::env::var("FITNESS_DEBUG_PROMPT").is_ok() {
        println!("===================================================");
        println!("{}", brief);
        println!("===================================================");
    }

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
                        let parsed: serde_json::Value = match serde_json::from_str(&json_str) {
                            Ok(v) => v,
                            Err(e) => {
                                eprintln!("Failed to parse generated JSON: {}", e);
                                return Ok(());
                            }
                        };

                        let workouts = if let Some(arr) = parsed.as_array() {
                            arr.clone()
                        } else {
                            vec![parsed]
                        };

                        let mut generated_count = 0;
                        let mut scheduled_details = Vec::new();
                        for w in workouts {
                            let mut workout_spec = w;
                            if let Some(obj) = workout_spec.as_object_mut() {
                                let current_name = obj
                                    .get("workoutName")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("Imported Strength Workout");
                                obj.insert(
                                    "workoutName".to_string(),
                                    serde_json::Value::String(
                                        crate::garmin_client::ensure_ai_workout_name(current_name),
                                    ),
                                );
                            }

                            if let Some(name) =
                                workout_spec.get("workoutName").and_then(|n| n.as_str())
                            {
                                println!("Creating workout: {}...", name);
                            }

                            let mut payload = builder.build_workout_payload(&workout_spec, false);
                            let mut workout_id = None;

                            // Trying normal payload
                            match garmin_client
                                .api
                                .connectapi_post("/workout-service/workout", &payload)
                                .await
                            {
                                Ok(res) => {
                                    println!("Garmin create response: {}", res);
                                    if let Some(id) = res.get("workoutId").and_then(|i| i.as_i64())
                                    {
                                        workout_id = Some(id);
                                        println!("Success! Workout ID: {}", id);
                                    } else {
                                        println!("Warning: workoutId not found in response.");
                                    }
                                }
                                Err(e) => {
                                    if e.to_string().contains("400") {
                                        println!("Failed with CSV mapping (400). Retrying with generic fallback...");
                                        payload =
                                            builder.build_workout_payload(&workout_spec, true);
                                        match garmin_client
                                            .api
                                            .connectapi_post("/workout-service/workout", &payload)
                                            .await
                                        {
                                            Ok(res) => {
                                                println!("Garmin generic create response: {}", res);
                                                if let Some(id) =
                                                    res.get("workoutId").and_then(|i| i.as_i64())
                                                {
                                                    workout_id = Some(id);
                                                    println!(
                                                        "Success! (Generic Mode) Workout ID: {}",
                                                        id
                                                    );
                                                } else {
                                                    println!("Warning: workoutId not found in generic response.");
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

                            if let (Some(id), Some(sch_date)) = (
                                workout_id,
                                workout_spec.get("scheduledDate").and_then(|d| d.as_str()),
                            ) {
                                println!("Scheduling workout {} on {}...", id, sch_date);
                                let sched_payload = serde_json::json!({ "date": sch_date });
                                let sched_endpoint = format!("/workout-service/schedule/{}", id);
                                match garmin_client
                                    .api
                                    .connectapi_post(&sched_endpoint, &sched_payload)
                                    .await
                                {
                                    Ok(_) => {
                                        println!("Successfully scheduled on {}", sch_date);
                                        generated_count += 1;
                                        let detailed_str =
                                            crate::bot::format_workout_details(&workout_spec);
                                        scheduled_details.push(format!(
                                            "📅 Scheduled for: {}\n{}",
                                            sch_date, detailed_str
                                        ));
                                    }
                                    Err(e) => println!("Failed to schedule: {}", e),
                                }
                            }
                        }

                        if generated_count > 0 {
                            let mut msg = format!("✅ AI Coach has successfully generated and scheduled {} new workouts!", generated_count);
                            if !scheduled_details.is_empty() {
                                msg.push_str("\n\n");
                                msg.push_str(&scheduled_details.join("\n\n"));
                            }
                            crate::bot::broadcast_message(&msg).await;
                        }

                        let _ = database.lock().await.clear_garmin_cache();
                    }
                    Err(e) => {
                        eprintln!("Could not extract JSON from AI response: {}", e);
                        if std::env::var("FITNESS_DEBUG_PROMPT").is_ok() {
                            println!("Raw Response:\n{}", markdown_response);
                        }
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

fn write_secret_json_file<T: serde::Serialize>(
    path: &str,
    value: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::write(path, serde_json::to_string_pretty(value)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}
