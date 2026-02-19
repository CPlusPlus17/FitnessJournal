mod models;
mod grafana_client;
mod garmin_client;
mod coaching;
mod ai_client;
mod db;
mod api;
mod bot;

use std::sync::Arc;
use tokio::sync::Mutex;
use crate::garmin_client::GarminClient;
use crate::coaching::Coach;
use crate::db::Database;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    println!("Starting Fitness Coach...");

    // 1. Fetch History from InfluxDB (via Grafana)
    // Default to the known working IP/Token if not set
    let grafana_url = std::env::var("GRAFANA_URL").unwrap_or_else(|_| "http://10.0.15.4:3000".to_string());
    let grafana_token = std::env::var("GRAFANA_TOKEN").unwrap_or_else(|_| "***REMOVED***".to_string());
    
    // Initialize components
    let client = Arc::new(crate::grafana_client::GrafanaClient::new(grafana_url.clone(), grafana_token.clone()));
    let garmin_client = Arc::new(GarminClient::new());
    let coach = Arc::new(Coach::new());
    
    // Initialize Database
    let database = Arc::new(Mutex::new(Database::new().expect("Failed to initialize SQLite database")));
    

    let args: Vec<String> = std::env::args().collect();
    let is_daemon = args.len() > 1 && args.contains(&"--daemon".to_string());
    let is_signal = args.len() > 1 && args.contains(&"--signal".to_string());
    let is_api = args.len() > 1 && args.contains(&"--api".to_string());

    if is_api {
        println!("Starting Fitness Coach in API mode.");
        if let Err(e) = api::run_server(database.clone(), client.clone(), garmin_client.clone(), coach.clone()).await {
             eprintln!("API Server crashed: {}", e);
        }
        return Ok(());
    }

    if is_signal {
        let bot = bot::BotController::new(
            client.clone(),
            garmin_client.clone(),
            coach.clone(),
            database.clone()
        );
        bot.run().await;
        return Ok(());
    }

    if is_daemon {
        println!("Starting Fitness Coach in DAEMON mode. Will run every 24 hours.");
        loop {
            run_coach_pipeline(client.clone(), garmin_client.clone(), coach.clone(), database.clone()).await?;
            println!("Sleeping for 24 hours... zzz");
            tokio::time::sleep(tokio::time::Duration::from_secs(86400)).await;
        }
    } else {
        run_coach_pipeline(client.clone(), garmin_client.clone(), coach.clone(), database.clone()).await?;
    }

    Ok(())
}

pub async fn run_coach_pipeline(
    client: Arc<crate::grafana_client::GrafanaClient>,
    garmin_client: Arc<GarminClient>,
    coach: Arc<Coach>,
    database: Arc<Mutex<Database>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut activities = Vec::new();

    // 1. Fetch High-Level Data from InfluxDB
    if let Err(e) = client.test_connection().await {
        eprintln!("Warning: Could not connect to Grafana: {}", e);
    } else {
        println!("Fetching summary activities from InfluxDB...");
        match client.get_recent_activities(30).await {
            Ok(acts) => {
                println!("Found {} summary activities.", acts.len());
                activities = acts;
            },
            Err(e) => eprintln!("Failed to fetch activities: {}", e),
        }
    }

    // 2. Fetch Detailed Data from Garmin Connect (Python Bridge)
    println!("\nFetching detailed stats from Garmin Connect...");
    let (detailed_activities, active_plans, user_profile, max_metrics, scheduled_workouts, recovery) = match garmin_client.fetch_data() {
        Ok(response) => {
            println!("Found {} detailed activities, {} active plans, and {} scheduled workouts.", 
                response.activities.len(), response.plans.len(), response.scheduled_workouts.len());
            (response.activities, response.plans, response.user_profile, response.max_metrics, response.scheduled_workouts, response.recovery_metrics)
        },
        Err(e) => {
            eprintln!("Failed to fetch detailed Garmin data: {}", e);
            (Vec::new(), Vec::new(), None, None, Vec::new(), None)
        }
    };
    
    // --- 2b. Sync Garmin Strength Sets to Local Database ---
    for act in &detailed_activities {
        if let Err(e) = database.lock().await.insert_activity(act) {
            eprintln!("Failed to insert activity {} into DB: {}", act.id, e);
        }
    }
    
    // Fetch 1RM/3RM/10RM History
    let progression_history = database.lock().await.get_progression_history().unwrap_or_default();
    println!("Loaded progression history for {} exercises.", progression_history.len());

    // 3. Define Context & Goals
    let mut context = crate::coaching::CoachContext {
        goals: vec![
            "Improve Marathon Time (Sub 4h)".to_string(),
            "Maintain Upper Body Strength (Hypertrophy)".to_string(),
            "Increase VO2Max".to_string(),
        ],
        constraints: vec![],
        available_equipment: vec![]
    };

    // Load active profile from profiles.json
    if let Ok(profile_data) = std::fs::read_to_string("profiles.json") {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&profile_data) {
            if let Some(active_name) = json.get("active_profile").and_then(|v| v.as_str()) {
                println!("Loaded active equipment profile: {}", active_name);
                if let Some(profile) = json.get("profiles").and_then(|p| p.get(active_name)) {
                    if let Some(constraints) = profile.get("constraints").and_then(|c| c.as_array()) {
                        context.constraints = constraints.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
                    }
                    if let Some(equipment) = profile.get("available_equipment").and_then(|e| e.as_array()) {
                        context.available_equipment = equipment.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
                    }
                }
            }
        }
    }

    // Generate Brief
    println!("\nGenerating Coach Brief...");
    let brief = coach.generate_brief(
        &activities, 
        &detailed_activities, 
        &active_plans,
        &user_profile,
        &max_metrics,
        &scheduled_workouts,
        &recovery,
        &context,
        &progression_history
    );

    println!("===================================================");
    println!("{}", brief);
    println!("===================================================");

    // Phase 2: AI Execution & Auto-Upload
    if let Ok(gemini_key) = std::env::var("GEMINI_API_KEY") {
        println!("\nGEMINI_API_KEY found! Generating workout via Gemini 3.1 Pro Preview...");
        
        // Initialize AI Client
        let ai_client = crate::ai_client::AiClient::new(gemini_key);
        
        match ai_client.generate_workout(&brief).await {
            Ok(markdown_response) => {
                println!("Received response from AI!");
                
                match crate::ai_client::AiClient::extract_json_block(&markdown_response) {
                    Ok(json_str) => {
                        let out_file = "generated_workouts.json";
                        std::fs::write(out_file, &json_str)?;
                        println!("Saved structured workout to {}", out_file);

                        // Upload to Garmin
                        println!("Uploading to Garmin Connect...");
                        let output = std::process::Command::new(".venv/bin/python3")
                            .arg("scripts/create_workout.py")
                            .arg(out_file)
                            .output()?;

                        if output.status.success() {
                            println!("{}", String::from_utf8_lossy(&output.stdout));
                        } else {
                            eprintln!("Failed to upload workout:\n{}", String::from_utf8_lossy(&output.stderr));
                        }
                    },
                    Err(e) => {
                        eprintln!("Could not extract JSON from AI response: {}", e);
                        println!("Raw Response:\n{}", markdown_response);
                    }
                }
            },
            Err(e) => eprintln!("Failed to call Gemini: {}", e),
        }
    } else {
        println!("\nNo GEMINI_API_KEY set. Skipping automatic workout generation.");
    }

    Ok(())
}
