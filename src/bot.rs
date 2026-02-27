use futures_util::StreamExt;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as WsMessage};

use crate::coaching::Coach;
use crate::db::Database;
use crate::garmin_client::GarminClient;

pub struct BotController {
    garmin_client: Arc<GarminClient>,
    coach: Arc<Coach>,
    database: Arc<Mutex<Database>>,
}

// Structs removed in favor of serde_json::Value

#[derive(Serialize)]
struct SendMessageReq {
    message: String,
    number: String,
    recipients: Vec<String>,
}

impl BotController {
    pub fn new(
        garmin_client: Arc<GarminClient>,
        coach: Arc<Coach>,
        database: Arc<Mutex<Database>>,
    ) -> Self {
        Self {
            garmin_client,
            coach,
            database,
        }
    }

    pub async fn run(&self) {
        println!("Starting Signal Bot... connecting to signal-cli-rest-api WS...");

        let signal_number = match std::env::var("SIGNAL_PHONE_NUMBER") {
            Ok(n) if !n.trim().is_empty() => n,
            _ => {
                eprintln!("CRITICAL: SIGNAL_PHONE_NUMBER environment variable is missing but bot was started. Exiting bot loop.");
                return;
            }
        };

        // Use environment variable for the host, defaulting to the docker-compose service name
        let api_host = std::env::var("SIGNAL_API_HOST")
            .unwrap_or_else(|_| "fitness-coach-signal-api".to_string());
        let ws_url = format!("ws://{}:8080/v1/receive/{}", api_host, signal_number);

        let (ws_stream, _) = match connect_async(&ws_url).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "Failed to connect to Signal WebSocket. Is the docker container running? {}",
                    e
                );
                return;
            }
        };

        println!("Signal Bot Connected!");
        let (mut _write, mut read) = ws_stream.split();
        let mut processed_msgs = std::collections::VecDeque::new();

        while let Some(msg) = read.next().await {
            if let Ok(WsMessage::Text(text)) = msg {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                    let mut text_content = None;
                    let mut sender = None;
                    let mut timestamp = 0;

                    if let Some(envelope) = parsed.get("envelope") {
                        if let Some(source) = envelope.get("source").and_then(|s| s.as_str()) {
                            sender = Some(source.to_string());
                        } else if let Some(source_num) = envelope.get("sourceNumber").and_then(|s| s.as_str()) {
                            sender = Some(source_num.to_string());
                        } else if let Some(account) = parsed.get("account").and_then(|s| s.as_str()) {
                            sender = Some(account.to_string());
                        }

                        timestamp = envelope
                            .get("timestamp")
                            .and_then(|t| t.as_u64())
                            .unwrap_or(0);

                        // Normal messages
                        if let Some(data_message) = envelope.get("dataMessage") {
                            if let Some(msg_text) =
                                data_message.get("message").and_then(|m| m.as_str())
                            {
                                text_content = Some(msg_text.to_string());
                            }
                        }

                        // Note to self / linked device messages (syncMessage)
                        if let Some(sync_message) = envelope.get("syncMessage") {
                            if let Some(sent_message) = sync_message.get("sentMessage") {
                                if let Some(msg_text) =
                                    sent_message.get("message").and_then(|m| m.as_str())
                                {
                                    let destination = sent_message.get("destination").and_then(|d| d.as_str());
                                    let destination_num = sent_message.get("destinationNumber").and_then(|d| d.as_str());
                                    let destination_uuid = sent_message.get("destinationUuid").and_then(|d| d.as_str());
                                    let account = parsed.get("account").and_then(|a| a.as_str());
                                    let source = envelope.get("source").and_then(|s| s.as_str());
                                    let source_uuid = envelope.get("sourceUuid").and_then(|s| s.as_str());
                                    
                                    let is_note_to_self = 
                                        (destination.is_some() && destination == account) ||
                                        (destination_num.is_some() && destination_num == account) ||
                                        (destination.is_some() && destination == source) ||
                                        (destination_uuid.is_some() && destination_uuid == source_uuid && source_uuid.is_some());

                                    if is_note_to_self {
                                        text_content = Some(msg_text.to_string());
                                        // Ensure sender is the account so we reply correctly to Note to Self
                                        if let Some(acc) = account {
                                            sender = Some(acc.to_string());
                                        }
                                    } else {
                                        println!("Ignoring sent message to foreign destination: {:?}", destination);
                                    }
                                }
                            }
                        }
                    }

                    if let (Some(msg_text), Some(msg_sender)) = (text_content, sender) {
                        let text_trim = msg_text.trim();
                        let msg_id = format!("{}_{}", msg_sender, timestamp);

                        if processed_msgs.contains(&msg_id) {
                            continue; // Deduplicate re-delivered or sync+data duplication
                        }
                        processed_msgs.push_back(msg_id.clone());
                        if processed_msgs.len() > 100 {
                            processed_msgs.pop_front();
                        }

                        println!("Received Signal message from {}", msg_sender);

                        if text_trim.starts_with('/') {
                            let mut parts = text_trim.splitn(2, ' ');
                            let cmd = parts.next().unwrap_or("");
                            let args = parts.next().unwrap_or("").trim();

                            let response = self.handle_command(cmd, args).await;
                            self.send_reply(&msg_sender, &response).await;
                        }
                    }
                }
            }
        }
    }

    async fn handle_command(&self, cmd: &str, args: &str) -> String {
        match cmd {
            "/status" => match self.garmin_client.fetch_data().await {
                Ok(data) => {
                    let bb = data
                        .recovery_metrics
                        .as_ref()
                        .and_then(|m| m.current_body_battery)
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "N/A".to_string());
                    let sleep = data
                        .recovery_metrics
                        .as_ref()
                        .and_then(|m| m.sleep_score)
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "N/A".to_string());
                    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
                    let today_workouts: Vec<_> = data
                        .scheduled_workouts
                        .iter()
                        .filter(|w| w.date.starts_with(&today))
                        .collect();

                    let planned_str = if today_workouts.is_empty() {
                        "None - Rest Day!".to_string()
                    } else {
                        today_workouts
                            .iter()
                            .map(|w| {
                                format!(
                                    "{} ({})",
                                    w.title.as_deref().unwrap_or("Untitled"),
                                    w.sport.as_deref().unwrap_or("Unknown")
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    };

                    format!("📊 Current Status\n\n🔋 Body Battery: {}/100\n😴 Sleep Score: {}/100\n\n📅 Today's Plan: {}", bb, sleep, planned_str)
                }
                Err(e) => format!("Failed to fetch status from Garmin: {}", e),
            },
            "/generate" => {
                match crate::run_coach_pipeline(
                    self.garmin_client.clone(),
                    self.coach.clone(),
                    self.database.clone(),
                )
                .await
                {
                    Ok(_) => {
                        "✅ Successfully generated and scheduled the week's workouts!".to_string()
                    }
                    Err(e) => format!("Failed to generate workout: {}", e),
                }
            }
            "/macros" => {
                if args.is_empty() {
                    "Please provide macros. Example: /macros 2500 150 (calories protein)"
                        .to_string()
                } else {
                    let parts: Vec<&str> = args.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let kcal_str = parts[0].replace("kcal", "");
                        let protein_str = parts[1].replace("g", "");

                        if let (Ok(kcal), Ok(protein)) =
                            (kcal_str.parse::<i32>(), protein_str.parse::<i32>())
                        {
                            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
                            let db = self.database.lock().await;
                            if let Err(e) = db.log_nutrition(&today, kcal, protein) {
                                format!("Failed to log macros: {}", e)
                            } else {
                                format!("✅ Logged Macros: {} kcal, {}g protein.", kcal, protein)
                            }
                        } else {
                            "Invalid number format. Example: /macros 2500 150".to_string()
                        }
                    } else {
                        "Invalid format. Example: /macros 2500 150".to_string()
                    }
                }
            }
            _ => "Command not recognized. Use /status, /generate, or /macros.".to_string(),
        }
    }

    async fn send_reply(&self, recipient: &str, text: &str) {
        let phone_number = match std::env::var("SIGNAL_PHONE_NUMBER") {
            Ok(n) if !n.trim().is_empty() => n,
            _ => {
                eprintln!("Warning: SIGNAL_PHONE_NUMBER not set. Cannot send reply.");
                return;
            }
        };

        let send_req = SendMessageReq {
            message: text.to_string(),
            number: phone_number,
            recipients: vec![recipient.to_string()],
        };

        let api_host = std::env::var("SIGNAL_API_HOST")
            .unwrap_or_else(|_| "fitness-coach-signal-api".to_string());
        let client = reqwest::Client::new();
        let res = client
            .post(format!("http://{}:8080/v2/send", api_host))
            .json(&send_req)
            .send()
            .await;

        match res {
            Ok(r) => {
                if !r.status().is_success() {
                    let status = r.status();
                    if let Ok(body) = r.text().await {
                        eprintln!("Signal reply failed with status {}: {}", status, body);
                    } else {
                        eprintln!("Signal reply failed with status {}", status);
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to send Signal reply network error: {}", e);
            }
        }
    }
}

pub async fn broadcast_message(text: &str) {
    let subscribers_var = std::env::var("SIGNAL_SUBSCRIBERS").unwrap_or_default();
    if subscribers_var.trim().is_empty() {
        return;
    }

    let recipients: Vec<String> = subscribers_var
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if recipients.is_empty() {
        return;
    }
    
    let phone_number = match std::env::var("SIGNAL_PHONE_NUMBER") {
        Ok(n) if !n.trim().is_empty() => n,
        _ => {
            eprintln!("Warning: SIGNAL_PHONE_NUMBER not set. Skipping broadcast.");
            return;
        }
    };

    let send_req = SendMessageReq {
        message: text.to_string(),
        number: phone_number,
        recipients,
    };

    let api_host = std::env::var("SIGNAL_API_HOST")
        .unwrap_or_else(|_| "fitness-coach-signal-api".to_string());
    let client = reqwest::Client::new();
    let res = client
        .post(format!("http://{}:8080/v2/send", api_host))
        .json(&send_req)
        .send()
        .await;

    match res {
        Ok(r) => {
            if !r.status().is_success() {
                let status = r.status();
                if let Ok(body) = r.text().await {
                    eprintln!("Signal broadcast failed with status {}: {}", status, body);
                } else {
                    eprintln!("Signal broadcast failed with status {}", status);
                }
            } else {
                println!("Signal broadcast succeeded!");
            }
        }
        Err(e) => {
            eprintln!("Failed to broadcast Signal message network error: {}", e);
        }
    }
}

pub fn format_workout_details(workout_spec: &serde_json::Value) -> String {
    let mut out = String::new();
    let name = workout_spec.get("workoutName").and_then(|v| v.as_str()).unwrap_or("Unknown Workout");
    
    let display_name = crate::garmin_client::ensure_ai_workout_name(name);
    out.push_str(&format!("🏋️ {}\n", display_name));
    
    if let Some(desc) = workout_spec.get("description").and_then(|v| v.as_str()) {
        out.push_str(&format!("{}\n", desc));
    }
    if let Some(steps) = workout_spec.get("steps").and_then(|v| v.as_array()) {
        if !steps.is_empty() {
            out.push_str("\nSteps:\n");
            for step in steps {
                let exercise = step.get("exercise").and_then(|v| v.as_str()).unwrap_or("Activity");
                let phase = step.get("phase").and_then(|v| v.as_str()).unwrap_or("");
                let mut details = format!("- [{}] {}", phase.to_uppercase(), exercise);
                
                if let Some(dur) = step.get("duration").and_then(|v| v.as_str()) {
                    details.push_str(&format!(" ({})", dur));
                } else if let Some(dur_int) = step.get("duration").and_then(|v| v.as_i64()) {
                    details.push_str(&format!(" ({} mins)", dur_int));
                }
                if let Some(reps) = step.get("reps") {
                    let r = if reps.is_string() { reps.as_str().unwrap().to_string() } else { reps.to_string() };
                    details.push_str(&format!(" | Reps: {}", r));
                }
                if let Some(sets) = step.get("sets") {
                    details.push_str(&format!(" | Sets: {}", sets));
                }
                if let Some(weight) = step.get("weight") {
                    let w = if weight.is_string() { weight.as_str().unwrap().to_string() } else { weight.to_string() };
                    if w != "0" && w != "0.0" {
                       details.push_str(&format!(" | Weight: {}kg", w));
                    }
                }
                if let Some(note) = step.get("note").and_then(|v| v.as_str()) {
                    details.push_str(&format!("\n  📝 {}", note));
                }
                out.push_str(&details);
                out.push('\n');
            }
        }
    }
    out
}

pub fn start_morning_notifier(garmin_client: Arc<GarminClient>) {
    tokio::spawn(async move {
        let mut last_sent_date = String::new();

        loop {
            let now = chrono::Local::now();
            let today = now.format("%Y-%m-%d").to_string();

            let time_str = std::env::var("MORNING_MESSAGE_TIME")
                .unwrap_or_else(|_| "07:00".to_string());

            let current_time = now.format("%H:%M").to_string();

            if current_time == time_str && last_sent_date != today {
                match garmin_client.fetch_data().await {
                    Ok(data) => {
                        let today_workouts: Vec<_> = data
                            .scheduled_workouts
                            .iter()
                            .filter(|w| w.date.starts_with(&today))
                            .collect();

                        if !today_workouts.is_empty() {
                            let planned_str = today_workouts
                                .iter()
                                .map(|w| {
                                    format!(
                                        "{} ({})",
                                        w.title.as_deref().unwrap_or("Untitled"),
                                        w.sport.as_deref().unwrap_or("Unknown")
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join("\n- ");

                            let msg = format!(
                                "🌅 Good morning! You have workouts scheduled for today:\n- {}",
                                planned_str
                            );
                            broadcast_message(&msg).await;
                        }

                        last_sent_date = today;
                    }
                    Err(e) => {
                        eprintln!("Morning notifier failed to fetch garmin data: {}", e);
                    }
                }
            }

            // Sleep for roughly a minute
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    });
}
