use figment::{
    providers::{Env, Format, Json, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub database_url: String,

    // Signal Bot Settings
    #[serde(default)]
    pub signal_phone_number: String,
    pub signal_api_host: String,
    #[serde(default)]
    pub signal_subscribers: String,
    pub morning_message_time: String,
    pub readiness_message_time: String,
    pub weekly_review_day: String,
    pub weekly_review_time: String,
    pub monthly_review_day: u32,
    pub monthly_review_time: String,
    pub force_monthly_debrief: bool,

    // API Settings
    pub cors_allowed_origins: String,
    pub api_auth_token: Option<String>,
    pub api_bind_addr: String,
    pub chat_rate_limit_per_minute: usize,
    pub generate_rate_limit_per_hour: usize,

    // AI/Gemini Settings
    pub gemini_api_key: String,
    pub fitness_debug_prompt: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            database_url: "fitness_journal.db".to_string(),
            signal_phone_number: "".to_string(),
            signal_api_host: "fitness-coach-signal-api".to_string(),
            signal_subscribers: "".to_string(),
            morning_message_time: "07:00".to_string(),
            readiness_message_time: "08:00".to_string(),
            weekly_review_day: "Sun".to_string(),
            weekly_review_time: "18:00".to_string(),
            monthly_review_day: 1,
            monthly_review_time: "18:00".to_string(),
            force_monthly_debrief: false,
            cors_allowed_origins: "http://localhost:3000".to_string(),
            api_auth_token: None,
            api_bind_addr: "127.0.0.1:3001".to_string(),
            chat_rate_limit_per_minute: 30,
            generate_rate_limit_per_hour: 6,
            gemini_api_key: "".to_string(),
            fitness_debug_prompt: false,
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self, figment::Error> {
        let mut config: AppConfig = Figment::from(figment::providers::Serialized::defaults(
            AppConfig::default(),
        ))
        .merge(Toml::file("Fitness.toml"))
        .merge(Json::file("Fitness.json"))
        .merge(Env::raw().ignore(&["SIGNAL_PHONE_NUMBER", "SIGNAL_SUBSCRIBERS"]))
        .extract()?;

        if let Ok(num) = std::env::var("SIGNAL_PHONE_NUMBER") {
            config.signal_phone_number = num;
        }
        if let Ok(subs) = std::env::var("SIGNAL_SUBSCRIBERS") {
            config.signal_subscribers = subs;
        }

        Ok(config)
    }
}
