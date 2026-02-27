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
                        } else if let Some(source_num) =
                            envelope.get("sourceNumber").and_then(|s| s.as_str())
                        {
                            sender = Some(source_num.to_string());
                        } else if let Some(account) = parsed.get("account").and_then(|s| s.as_str())
                        {
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
                                    let destination =
                                        sent_message.get("destination").and_then(|d| d.as_str());
                                    let destination_num = sent_message
                                        .get("destinationNumber")
                                        .and_then(|d| d.as_str());
                                    let destination_uuid = sent_message
                                        .get("destinationUuid")
                                        .and_then(|d| d.as_str());
                                    let account = parsed.get("account").and_then(|a| a.as_str());
                                    let source = envelope.get("source").and_then(|s| s.as_str());
                                    let source_uuid =
                                        envelope.get("sourceUuid").and_then(|s| s.as_str());

                                    let is_note_to_self = (destination.is_some()
                                        && destination == account)
                                        || (destination_num.is_some()
                                            && destination_num == account)
                                        || (destination.is_some() && destination == source)
                                        || (destination_uuid.is_some()
                                            && destination_uuid == source_uuid
                                            && source_uuid.is_some());

                                    if is_note_to_self {
                                        text_content = Some(msg_text.to_string());
                                        // Ensure sender is the account so we reply correctly to Note to Self
                                        if let Some(acc) = account {
                                            sender = Some(acc.to_string());
                                        }
                                    } else {
                                        println!(
                                            "Ignoring sent message to foreign destination: {:?}",
                                            destination
                                        );
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
                        } else {
                            // Conversational Logic
                            let response = self.handle_conversation(text_trim).await;
                            self.send_reply(&msg_sender, &response).await;
                        }
                    }
                }
            }
        }
    }

    async fn handle_conversation(&self, text: &str) -> String {
        let gemini_key = match std::env::var("GEMINI_API_KEY") {
            Ok(k) if !k.is_empty() => k,
            _ => return "I cannot respond contextually without a GEMINI_API_KEY.".to_string(),
        };

        // 1. Fetch live context silently
        let mut context_str = String::new();
        if let Ok(data) = self.garmin_client.fetch_data().await {
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

            context_str = format!(
                "Body Battery: {}\nSleep Score: {}\nToday's Planned Workouts: {}",
                bb, sleep, planned_str
            );
        }

        let ai_client = crate::ai_client::AiClient::new(gemini_key);

        {
            let db = self.database.lock().await;
            let _ = db.add_ai_chat_message("user", text);
        }

        let history = {
            let db = self.database.lock().await;
            db.get_ai_chat_history().unwrap_or_default()
        };

        match ai_client
            .chat_with_history(&history, Some(&context_str))
            .await
        {
            Ok(response) => {
                {
                    let db = self.database.lock().await;
                    let _ = db.add_ai_chat_message("model", &response);
                }

                // Scan for JSON code block indicating a reschedule
                if let Ok(json_str) = crate::ai_client::AiClient::extract_json_block(&response) {
                    if let Ok(workouts) = serde_json::from_str::<Vec<serde_json::Value>>(&json_str)
                    {
                        for workout_spec in workouts {
                            crate::workout_builder::WorkoutBuilder::new()
                                .build_workout_payload(&workout_spec, true);
                            match self
                                .garmin_client
                                .create_and_schedule_workout(&workout_spec)
                                .await
                            {
                                Ok(msg) => {
                                    println!("Conversational Coach Scheduled Workout: {}", msg)
                                }
                                Err(e) => {
                                    eprintln!("Conversational Coach failed to schedule: {}", e)
                                }
                            }
                        }
                    }
                }

                // Strip the exact markdown json block from the response before sending it
                let clean_response = if let Some(start_idx) = response.find("```json") {
                    if let Some(end_idx) = response[start_idx..]
                        .find("```\n")
                        .or_else(|| response[start_idx..].find("```"))
                    {
                        let full_end = start_idx + end_idx + 3;
                        let mut cleaned = response.clone();
                        // Also remove a trailing newline if it exists right after the block
                        if cleaned.len() > full_end && cleaned.as_bytes()[full_end] == b'\n' {
                            cleaned.replace_range(start_idx..=full_end, "");
                        } else {
                            cleaned.replace_range(start_idx..full_end, "");
                        }
                        cleaned.trim().to_string()
                    } else {
                        response
                    }
                } else {
                    response
                };

                clean_response
            }
            Err(e) => format!("My coaching brain failed to connect: {}", e),
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

    let api_host =
        std::env::var("SIGNAL_API_HOST").unwrap_or_else(|_| "fitness-coach-signal-api".to_string());
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
    let name = workout_spec
        .get("workoutName")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown Workout");

    let display_name = crate::garmin_client::ensure_ai_workout_name(name);
    out.push_str(&format!("🏋️ {}\n", display_name));

    if let Some(desc) = workout_spec.get("description").and_then(|v| v.as_str()) {
        out.push_str(&format!("{}\n", desc));
    }
    if let Some(steps) = workout_spec.get("steps").and_then(|v| v.as_array()) {
        if !steps.is_empty() {
            out.push_str("\nSteps:\n");
            for step in steps {
                let exercise = step
                    .get("exercise")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Activity");
                let phase = step.get("phase").and_then(|v| v.as_str()).unwrap_or("");
                let mut details = format!("- [{}] {}", phase.to_uppercase(), exercise);

                if let Some(dur) = step.get("duration").and_then(|v| v.as_str()) {
                    details.push_str(&format!(" ({})", dur));
                } else if let Some(dur_int) = step.get("duration").and_then(|v| v.as_i64()) {
                    details.push_str(&format!(" ({} mins)", dur_int));
                }
                if let Some(reps) = step.get("reps") {
                    let r = if reps.is_string() {
                        reps.as_str().unwrap().to_string()
                    } else {
                        reps.to_string()
                    };
                    details.push_str(&format!(" | Reps: {}", r));
                }
                if let Some(sets) = step.get("sets") {
                    details.push_str(&format!(" | Sets: {}", sets));
                }
                if let Some(weight) = step.get("weight") {
                    let w = if weight.is_string() {
                        weight.as_str().unwrap().to_string()
                    } else {
                        weight.to_string()
                    };
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

            let time_str =
                std::env::var("MORNING_MESSAGE_TIME").unwrap_or_else(|_| "07:00".to_string());

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

pub fn start_weekly_review_notifier(garmin_client: Arc<GarminClient>, gemini_key: String) {
    tokio::spawn(async move {
        let mut last_sent_week = String::new();

        loop {
            let now = chrono::Local::now();
            let today_str = now.format("%Y-%m-%d").to_string();
            // Get week representation like "2026-W09" to ensure we only send once per week
            let current_week = now.format("%G-W%V").to_string();

            let target_day =
                std::env::var("WEEKLY_REVIEW_DAY").unwrap_or_else(|_| "Sun".to_string());
            let current_day = now.format("%a").to_string(); // e.g. "Sun"

            let target_time =
                std::env::var("WEEKLY_REVIEW_TIME").unwrap_or_else(|_| "18:00".to_string());
            let current_time = now.format("%H:%M").to_string();

            if current_day == target_day
                && current_time == target_time
                && last_sent_week != current_week
            {
                match garmin_client.fetch_data().await {
                    Ok(data) => {
                        let ai_client = crate::ai_client::AiClient::new(gemini_key.clone());
                        let seven_days_ago = now - chrono::Duration::days(7);
                        let seven_days_ago_str = seven_days_ago.format("%Y-%m-%d").to_string();

                        let recent_activities: Vec<_> = data
                            .activities
                            .iter()
                            .filter(|a| a.start_time >= seven_days_ago_str)
                            .collect();

                        // Calculate basic Volume
                        let total_duration_mins: f64 = recent_activities
                            .iter()
                            .filter_map(|a| a.duration)
                            .sum::<f64>()
                            / 60.0;
                        let total_distance_km: f64 = recent_activities
                            .iter()
                            .filter_map(|a| a.distance)
                            .sum::<f64>()
                            / 1000.0;
                        let act_count = recent_activities.len();

                        // Build Prompt Context
                        let mut context = format!(
                            "Athlete's Weekly Summary\nTimeframe: {} to {}\nWorkouts Completed: {}\nTotal Duration: {:.1} mins\nTotal Distance: {:.1} km\n",
                            seven_days_ago_str, today_str, act_count, total_duration_mins, total_distance_km
                        );

                        if let Some(metrics) = &data.recovery_metrics {
                            let sleep = metrics
                                .sleep_score
                                .map_or("N/A".to_string(), |v| v.to_string());
                            let bb = metrics
                                .current_body_battery
                                .map_or("N/A".to_string(), |v| v.to_string());
                            let hrv = metrics.hrv_status.as_deref().unwrap_or("N/A");
                            context.push_str(&format!("\nCurrent Recovery Stats:\nSleep Score: {}\nBody Battery: {}\nHRV Status: {}\n", sleep, bb, hrv));
                        }

                        let tomorrow = (now + chrono::Duration::days(1))
                            .format("%Y-%m-%d")
                            .to_string();
                        let upcoming: Vec<_> = data
                            .scheduled_workouts
                            .iter()
                            .filter(|w| w.date.starts_with(&tomorrow))
                            .collect();

                        if !upcoming.is_empty() {
                            context.push_str("\nTomorrow's Schedule:\n");
                            for w in upcoming {
                                context.push_str(&format!(
                                    "- {} ({})\n",
                                    w.title.as_deref().unwrap_or("Workout"),
                                    w.sport.as_deref().unwrap_or("unknown")
                                ));
                            }
                        }

                        let prompt = format!(
                            "You are the athlete's elite performance coach. Review the following weekly summary of their Garmin data.\n\
                            Write a highly encouraging, crisp, 2-3 paragraph weekly review to be sent on Signal. \n\
                            Acknowledge their work volume, comment critically but kindly on any recovery trends (sleep, body battery), and give them a focal point for the upcoming week based on tomorrow's schedule.\n\
                            Keep the tone professional, motivating, and conversational.\n\n\
                            === WEEKLY DATA ===\n{}",
                            context
                        );

                        match ai_client.generate_workout(&prompt).await {
                            Ok(review) => {
                                let msg = format!("📈 **Weekly Coach Review**\n\n{}", review);
                                broadcast_message(&msg).await;
                                last_sent_week = current_week;
                            }
                            Err(e) => eprintln!("Failed to generate weekly review from AI: {}", e),
                        }
                    }
                    Err(e) => {
                        eprintln!("Weekly review notifier failed to fetch garmin data: {}", e);
                    }
                }
            }

            // Sleep for roughly a minute
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    });
}
