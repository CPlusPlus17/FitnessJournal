use futures_util::StreamExt;
use tracing::{info, error};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as WsMessage};

use crate::coaching::Coach;
use crate::db::Database;
use crate::garmin_client::GarminClient;
pub struct BotController {
    pub database: Arc<Mutex<Database>>,
    pub config: Arc<crate::config::AppConfig>,
    pub garmin_client: Arc<GarminClient>,
    pub coach: Arc<Coach>,
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
        config: Arc<crate::config::AppConfig>,
        garmin_client: Arc<GarminClient>,
        coach: Arc<Coach>,
        database: Arc<Mutex<Database>>,
    ) -> Self {
        Self {
            config,
            garmin_client,
            coach,
            database,
        }
    }

    pub async fn run(&self) {
        info!("Starting Signal Bot... connecting to signal-cli-rest-api WS...");

        let signal_number = &self.config.signal_phone_number;
        if signal_number.trim().is_empty() {
            error!("CRITICAL: signal_phone_number configuration is missing but bot was started. Exiting bot loop.");
            return;
        }

        let api_host = &self.config.signal_api_host;
        let ws_url = format!("ws://{}:8080/v1/receive/{}", api_host, signal_number);

        let (ws_stream, _) = match connect_async(&ws_url).await {
            Ok(s) => s,
            Err(e) => {
                error!(
                    "Failed to connect to Signal WebSocket. Is the docker container running? {}",
                    e
                );
                return;
            }
        };

        info!("Signal Bot Connected!");
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
                                        info!(
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

                        info!("Received Signal message from {}", msg_sender);

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
        let gemini_key = &self.config.gemini_api_key;
        if gemini_key.is_empty() {
            return "I cannot respond contextually without a GEMINI_API_KEY.".to_string();
        }

        // 1. Fetch live context silently
        let now = chrono::Local::now();
        let mut context_str = format!("Current Date: {}", now.format("%a, %Y-%m-%d %H:%M"));

        if let Ok(data) = self.garmin_client.fetch_data().await {
            let bb = data
                .recovery_metrics
                .as_ref()
                .and_then(|m| m.current_body_battery)
                .map(|v: i32| v.to_string())
                .unwrap_or_else(|| "N/A".to_string());
            let sleep = data
                .recovery_metrics
                .as_ref()
                .and_then(|m| m.sleep_score)
                .map(|v: i32| v.to_string())
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

            context_str.push_str(&format!(
                "\nBody Battery: {}\nSleep Score: {}\nToday's Planned Workouts: {}",
                bb, sleep, planned_str
            ));

            // Add recent activities to context
            let seven_days_ago = (chrono::Local::now() - chrono::Duration::days(7))
                .format("%Y-%m-%d")
                .to_string();
            let recent_activities: Vec<_> = data
                .activities
                .iter()
                .filter(|a| a.start_time >= seven_days_ago)
                .collect();

            if !recent_activities.is_empty() {
                context_str.push_str("\n\nRecent Workouts (Last 7 Days):\n");
                for act in recent_activities {
                    let name = act.name.as_deref().unwrap_or("Untitled");
                    let sport = act.get_activity_type().unwrap_or("Unknown");
                    let date = act.start_time.split('T').next().unwrap_or(&act.start_time);
                    let dist = act.distance.unwrap_or(0.0) / 1000.0;
                    let dur_mins = act.duration.unwrap_or(0.0) / 60.0;
                    context_str.push_str(&format!(
                        "- {} ({}) | {}: {:.1}km in {:.0} mins\n",
                        name, sport, date, dist, dur_mins
                    ));
                }
            }
        }

        // Add recent analyses to context
        {
            let db = self.database.lock().await;
            if let Ok(analyses) = db.get_recent_activity_analyses(7) {
                if !analyses.is_empty() {
                    context_str.push_str("\n\nRecent AI Coach Feedback (Last 7 Days):\n");
                    for (date, summary) in analyses {
                        context_str.push_str(&format!("- On {}:\n  {}\n", date, summary));
                    }
                }
            }
        }

        let ai_client = crate::ai_client::AiClient::new(gemini_key.to_string());

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
                    if let Ok(workouts) = serde_json::from_str::<Vec<serde_json::Value>>(&json_str) {
                        for workout_spec in workouts {
                            let _ = crate::workout_builder::WorkoutBuilder::new()
                                .build_workout_payload(&workout_spec, true);
                            info!("Conversational Coach Scheduled Workout");
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
                    self.config.clone(),
                    self.garmin_client.clone(),
                    self.coach.clone(),
                    self.database.clone(),
                    true,
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
            "/readiness" => {
                match self.garmin_client.fetch_data().await {
                    Ok(data) => {
                        if !self.config.gemini_api_key.is_empty() {
                            crate::bot::generate_race_readiness_assessment(&data, &self.config.gemini_api_key).await
                        } else {
                            "GEMINI_API_KEY is not set. Cannot run readiness assessment.".to_string()
                        }
                    }
                    Err(e) => format!("Failed to fetch Garmin data: {}", e),
                }
            }
            _ => "Command not recognized. Use /status, /generate, /readiness, or /macros.".to_string(),
        }
    }

    async fn send_reply(&self, recipient: &str, text: &str) {
        let phone_number = &self.config.signal_phone_number;
        if phone_number.trim().is_empty() {
            error!("Warning: signal_phone_number not set. Cannot send reply.");
            return;
        }

        let send_req = SendMessageReq {
            message: text.to_string(),
            number: phone_number.clone(),
            recipients: vec![recipient.to_string()],
        };

        let api_host = &self.config.signal_api_host;
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
                        error!("Signal reply failed with status {}: {}", status, body);
                    } else {
                        error!("Signal reply failed with status {}", status);
                    }
                }
            }
            Err(e) => {
                error!("Failed to send Signal reply network error: {}", e);
            }
        }
    }
}

pub async fn broadcast_message(text: &str, config: &crate::config::AppConfig) {
    let subscribers_var = &config.signal_subscribers;
    if subscribers_var.trim().is_empty() {
        return;
    }

    let recipients: Vec<String> = subscribers_var
        .split(',')
        .map(|s: &str| s.trim().to_string())
        .filter(|s: &String| !s.is_empty())
        .collect();

    if recipients.is_empty() {
        return;
    }

    let phone_number = &config.signal_phone_number;
    if phone_number.trim().is_empty() {
        error!("Warning: signal_phone_number not set. Skipping broadcast.");
        return;
    }

    let send_req = SendMessageReq {
        message: text.to_string(),
        number: phone_number.clone(),
        recipients,
    };

    let api_host = &config.signal_api_host;
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
                    error!("Signal broadcast failed with status {}: {}", status, body);
                } else {
                    error!("Signal broadcast failed with status {}", status);
                }
            } else {
                info!("Signal broadcast succeeded!");
            }
        }
        Err(e) => {
            error!("Failed to broadcast Signal message network error: {}", e);
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

pub fn start_morning_notifier(garmin_client: Arc<GarminClient>, config: Arc<crate::config::AppConfig>) {
    tokio::spawn(async move {
        let mut last_sent_date = String::new();

        loop {
            let now = chrono::Local::now();
            let today = now.format("%Y-%m-%d").to_string();

            let time_str = &config.morning_message_time;

            let current_time = now.format("%H:%M").to_string();

            if current_time == *time_str && last_sent_date != today {
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
                            broadcast_message(&msg, &config).await;
                        }

                        last_sent_date = today;
                    }
                    Err(e) => {
                        error!("Morning notifier failed to fetch garmin data: {}", e);
                    }
                }
            }

            // Sleep for roughly a minute
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    });
}

pub fn start_weekly_review_notifier(garmin_client: Arc<GarminClient>, config: Arc<crate::config::AppConfig>) {
    tokio::spawn(async move {
        let mut last_sent_week = String::new();

        loop {
            let now = chrono::Local::now();
            let today_str = now.format("%Y-%m-%d").to_string();
            // Get week representation like "2026-W09" to ensure we only send once per week
            let current_week = now.format("%G-W%V").to_string();

            let target_day = &config.weekly_review_day;
            let current_day = now.format("%a").to_string(); // e.g. "Sun"

            let target_time = &config.weekly_review_time;
            let current_time = now.format("%H:%M").to_string();

            if current_day == *target_day
                && current_time == *target_time
                && last_sent_week != current_week
            {
                match garmin_client.fetch_data().await {
                    Ok(data) => {
                        let ai_client = crate::ai_client::AiClient::new(config.gemini_api_key.clone());
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
                                broadcast_message(&msg, &config).await;
                                last_sent_week = current_week;
                            }
                            Err(e) => error!("Failed to generate weekly review from AI: {}", e),
                        }
                    }
                    Err(e) => {
                        error!("Weekly review notifier failed to fetch garmin data: {}", e);
                    }
                }
            }

            // Sleep for roughly a minute
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    });
}

pub async fn generate_race_readiness_assessment(data: &crate::models::GarminResponse, gemini_key: &str) -> String {
    let now = chrono::Local::now();
    let today_str = now.format("%Y-%m-%d").to_string();

    let mut upcoming_race: Option<crate::models::ScheduledWorkout> = None;
    for sw in &data.scheduled_workouts {
        if let Some(ref it) = sw.item_type {
            if it == "race" || it == "event" || it == "primaryEvent" {
                if sw.date >= today_str {
                    if upcoming_race.is_none() || sw.date < upcoming_race.as_ref().unwrap().date {
                        upcoming_race = Some(sw.clone());
                    }
                }
            }
        }
    }

    let race = match upcoming_race {
        Some(r) => r,
        None => return "No upcoming races or events found in your Garmin calendar.".to_string(),
    };

    let race_date = match chrono::NaiveDate::parse_from_str(&race.date, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return "Found a race but couldn't parse its date.".to_string(),
    };
    let today_date = now.naive_local().date();
    let days_until = (race_date - today_date).num_days();

    let twelve_weeks_ago = now - chrono::Duration::days(84);
    let twelve_weeks_ago_str = twelve_weeks_ago.format("%Y-%m-%d").to_string();

    let recent_activities: Vec<_> = data.activities.iter().filter(|a| a.start_time >= twelve_weeks_ago_str).collect();

    let total_dur_min: f64 = recent_activities.iter().filter_map(|a| a.duration).sum::<f64>() / 60.0;
    let total_dist_km: f64 = recent_activities.iter().filter_map(|a| a.distance).sum::<f64>() / 1000.0;
    let run_count = recent_activities.iter().filter(|a| a.get_activity_type().unwrap_or("").contains("run")).count();
    let bike_count = recent_activities.iter().filter(|a| a.get_activity_type().unwrap_or("").contains("biking") || a.get_activity_type().unwrap_or("").contains("cycl")).count();
    let strength_count = recent_activities.iter().filter(|a| a.get_activity_type().unwrap_or("").contains("strength") || a.get_activity_type().unwrap_or("").contains("fitness")).count();

    let mut recovery_str = String::new();
    if let Some(metrics) = &data.recovery_metrics {
        if let Some(bb) = metrics.current_body_battery { recovery_str.push_str(&format!("Body Battery: {}\n", bb)); }
        if let Some(ss) = metrics.sleep_score { recovery_str.push_str(&format!("Sleep Score: {}\n", ss)); }
        if let Some(hrv) = &metrics.hrv_status { recovery_str.push_str(&format!("HRV Status: {}\n", hrv)); }
    }

    let prompt = format!(
        "You are an elite sports coach. The athlete has an upcoming race in {} days!\n\
        \n=== EVENT DETAILS ===\n\
        Title: {}\n\
        Date: {}\n\
        Sport: {}\n\
        Distance: {:.1}km\n\
        \n=== 12-WEEK TRAINING BLOCK HISTORY ===\n\
        Total Duration: {:.1} hours\n\
        Total Distance: {:.1} km\n\
        Frequency: {} runs, {} rides, {} strength sessions\n\
        \n=== CURRENT RECOVERY ===\n\
        {}\n\
        \nProvide a 'Race Readiness Assessment' formatting it directly as text without markdown wrappers.\n\
        Include:\n\
        1. A Confidence Score (out of 100%).\n\
        2. Tapering advice given how many days are left ({} days).\n\
        3. A high-level pacing or mental strategy based on the distance.\n\
        Keep it highly encouraging, sharp, and focused purely on this event. Keep it concise enough for a messaging app (max 2-3 short paragraphs).",
        days_until,
        race.title.as_deref().unwrap_or("Untitled Event"),
        race.date,
        race.sport.as_deref().unwrap_or("Unknown"),
        race.distance.unwrap_or(0.0),
        total_dur_min / 60.0,
        total_dist_km,
        run_count, bike_count, strength_count,
        recovery_str,
        days_until
    );

    let ai_client = crate::ai_client::AiClient::new(gemini_key.to_string());
    match ai_client.generate_workout(&prompt).await {
        Ok(assessment) => format!("🏁 **Race Readiness Assessment**\n\n{}", assessment),
        Err(e) => format!("Failed to generate assessment: {}", e),
    }
}

pub fn start_race_readiness_notifier(garmin_client: Arc<GarminClient>, config: Arc<crate::config::AppConfig>) {
    tokio::spawn(async move {
        let mut last_notified_day = String::new();

        loop {
            let now = chrono::Local::now();
            let today_str = now.format("%Y-%m-%d").to_string();

            let current_time = now.format("%H:%M").to_string();
            let target_time = &config.readiness_message_time;

            if current_time == *target_time && last_notified_day != today_str {
                match garmin_client.fetch_data().await {
                    Ok(data) => {
                        let mut upcoming_race: Option<crate::models::ScheduledWorkout> = None;
                        for sw in &data.scheduled_workouts {
                            if let Some(ref it) = sw.item_type {
                                if it == "race" || it == "event" || it == "primaryEvent" {
                                    if sw.date >= today_str {
                                        if upcoming_race.is_none() || sw.date < upcoming_race.as_ref().unwrap().date {
                                            upcoming_race = Some(sw.clone());
                                        }
                                    }
                                }
                            }
                        }

                        if let Some(race) = upcoming_race {
                            if let Ok(race_date) = chrono::NaiveDate::parse_from_str(&race.date, "%Y-%m-%d") {
                                let today_date = now.naive_local().date();
                                let days_until = (race_date - today_date).num_days();

                                if days_until == 14 || days_until == 7 || days_until == 2 {
                                    let msg = generate_race_readiness_assessment(&data, &config.gemini_api_key).await;
                                    broadcast_message(&msg, &config).await;
                                }
                            }
                        }

                        last_notified_day = today_str;
                    }
                    Err(e) => {
                        error!("Race readiness notifier failed to fetch garmin data: {}", e);
                    }
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    });
}

pub fn start_monthly_debrief_notifier(garmin_client: Arc<GarminClient>, config: Arc<crate::config::AppConfig>) {
    tokio::spawn(async move {
        use chrono::Datelike;
        let mut last_sent_month = 0;

        loop {
            let now = chrono::Local::now();
            let current_day = now.day();
            let target_day = config.monthly_review_day;

            let current_time = now.format("%H:%M").to_string();
            let target_time = &config.monthly_review_time;
            let force = config.force_monthly_debrief;

            if (current_day == target_day && current_time == *target_time || force)
                && last_sent_month != now.month()
            {
                match garmin_client.fetch_data().await {
                    Ok(data) => {
                        let ai_client = crate::ai_client::AiClient::new(config.gemini_api_key.clone());
                        let year = now.year();
                        let month = now.month();

                        let (last_month_year, last_month) = if month == 1 {
                            (year - 1, 12)
                        } else {
                            (year, month - 1)
                        };

                        let (prev_month_year, prev_month) = if last_month == 1 {
                            (last_month_year - 1, 12)
                        } else {
                            (last_month_year, last_month - 1)
                        };

                        let last_month_prefix = format!("{}-{:02}", last_month_year, last_month);
                        let prev_month_prefix = format!("{}-{:02}", prev_month_year, prev_month);

                        let last_month_activities: Vec<_> = data
                            .activities
                            .iter()
                            .filter(|a| a.start_time.starts_with(&last_month_prefix))
                            .collect();

                        let prev_month_activities: Vec<_> = data
                            .activities
                            .iter()
                            .filter(|a| a.start_time.starts_with(&prev_month_prefix))
                            .collect();

                        // Last month volume
                        let lm_duration_hrs: f64 = last_month_activities
                            .iter()
                            .filter_map(|a| a.duration)
                            .sum::<f64>()
                            / 3600.0;
                        let lm_distance_km: f64 = last_month_activities
                            .iter()
                            .filter_map(|a| a.distance)
                            .sum::<f64>()
                            / 1000.0;
                        let lm_count = last_month_activities.len();

                        // Prev month volume
                        let pm_duration_hrs: f64 = prev_month_activities
                            .iter()
                            .filter_map(|a| a.duration)
                            .sum::<f64>()
                            / 3600.0;
                        let pm_distance_km: f64 = prev_month_activities
                            .iter()
                            .filter_map(|a| a.distance)
                            .sum::<f64>()
                            / 1000.0;
                        let pm_count = prev_month_activities.len();

                        // Strength tracking for 1RM
                        let mut max_weights = std::collections::HashMap::new();
                        for act in &last_month_activities {
                            if let Some(crate::models::GarminSetsData::Details(sets)) = &act.sets {
                                for set in &sets.exercise_sets {
                                    if let Some(w) = set.weight {
                                        for ex in &set.exercises {
                                            let current_max =
                                                max_weights.entry(ex.name.clone()).or_insert(0.0);
                                            if w > *current_max {
                                                *current_max = w;
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        let mut strength_summary = String::new();
                        let mut max_weights_vec: Vec<_> = max_weights.into_iter().collect();
                        max_weights_vec.sort_by(|a, b| {
                            b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
                        });
                        for (name, weight) in max_weights_vec.iter().take(10) {
                            strength_summary.push_str(&format!("- {}: {:.1}kg\n", name, weight));
                        }

                        let (context, _) = crate::load_profile_context();
                        let user_goals = if context.goals.is_empty() {
                            "General Fitness".to_string()
                        } else {
                            context.goals.join(", ")
                        };

                        let prompt = format!(
                            "You are an elite sports coach. Write a comprehensive Monthly Review to be sent on Signal.\n\
                            Compare total monthly volume, evaluate progress against the athlete's goals, and suggest focus blocks for the next macrocycle.\n\n\
                            === ATHLETE GOALS ===\n\
                            {}\n\n\
                            === LAST MONTH ({}) ===\n\
                            Workouts: {}\n\
                            Total Duration: {:.1} hours\n\
                            Total Distance: {:.1} km\n\n\
                            === PREVIOUS MONTH ({}) ===\n\
                            Workouts: {}\n\
                            Total Duration: {:.1} hours\n\
                            Total Distance: {:.1} km\n\n\
                            === PEAK WEIGHTS LIFTED (LAST MONTH) ===\n\
                            {}\n\n\
                            FORMAT:\n\
                            Keep it encouraging, analytical, and professional. 3-4 paragraphs max.\n\
                            Provide clear focus blocks for the upcoming month.",
                            user_goals,
                            last_month_prefix, lm_count, lm_duration_hrs, lm_distance_km,
                            prev_month_prefix, pm_count, pm_duration_hrs, pm_distance_km,
                            if strength_summary.is_empty() { "No strength data recorded.".to_string() } else { strength_summary }
                        );

                        match ai_client.generate_workout(&prompt).await {
                            Ok(review) => {
                                let msg = format!("📅 **Monthly Coach Debrief**\n\n{}", review);
                                broadcast_message(&msg, &config).await;
                                last_sent_month = now.month();
                            }
                            Err(e) => error!("Failed to generate monthly review from AI: {}", e),
                        }
                    }
                    Err(e) => {
                        error!("Monthly review notifier failed to fetch garmin data: {}", e);
                    }
                }
            }

            // Sleep for roughly a minute
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    });
}

