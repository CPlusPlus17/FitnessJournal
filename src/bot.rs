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

        let signal_number =
            std::env::var("SIGNAL_PHONE_NUMBER").unwrap_or_else(|_| "+1234567890".to_string());

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
                                    let account = parsed.get("account").and_then(|a| a.as_str());
                                    
                                    if destination == account || destination_num == account {
                                        text_content = Some(msg_text.to_string());
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
        let send_req = SendMessageReq {
            message: text.to_string(),
            number: std::env::var("SIGNAL_PHONE_NUMBER")
                .unwrap_or_else(|_| "+1234567890".to_string()),
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

        if let Err(e) = res {
            eprintln!("Failed to send Signal reply: {}", e);
        }
    }
}
