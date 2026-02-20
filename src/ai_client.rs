use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize)]
struct GeminiRequest {
    system_instruction: Option<SystemInstruction>,
    contents: Vec<Content>,
}

#[derive(Serialize)]
struct SystemInstruction {
    parts: Vec<Part>,
}

#[derive(Serialize)]
struct Content {
    role: String,
    parts: Vec<Part>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Part {
    text: String,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<Candidate>>,
    error: Option<GeminiError>,
}

#[derive(Deserialize)]
struct GeminiError {
    message: String,
}

#[derive(Deserialize)]
struct Candidate {
    content: ContentResponse,
}

#[derive(Deserialize)]
struct ContentResponse {
    parts: Vec<Part>,
}

pub struct AiClient {
    client: Client,
    api_key: String,
}

impl AiClient {
    pub fn new(api_key: String) -> Self {
        AiClient {
            client: Client::new(),
            api_key,
        }
    }

    pub async fn generate_workout(&self, prompt: &str) -> Result<String> {
        let request_body = GeminiRequest {
            system_instruction: Some(SystemInstruction {
                parts: vec![Part {
                    text: "You are an elite Multi-Sport Coach. Follow instructions precisely."
                        .to_string(),
                }],
            }),
            contents: vec![Content {
                role: "user".to_string(),
                parts: vec![Part {
                    text: prompt.to_string(),
                }],
            }],
        };

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-3.1-pro-preview:generateContent?key={}",
            self.api_key
        );

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let err_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Gemini API error: {} - {}", status, err_text));
        }

        let gemini_response: GeminiResponse = response
            .json()
            .await
            .context("Failed to parse Gemini JSON")?;

        if let Some(error) = gemini_response.error {
            return Err(anyhow!("Gemini returned an error: {}", error.message));
        }

        if let Some(candidates) = gemini_response.candidates {
            if let Some(candidate) = candidates.first() {
                if let Some(part) = candidate.content.parts.first() {
                    return Ok(part.text.clone());
                }
            }
        }

        Err(anyhow!("No valid content returned from Gemini"))
    }

    pub async fn chat_with_history(&self, history: &[(String, String)]) -> Result<String> {
        let mut contents = Vec::new();
        for (role, text) in history {
            contents.push(Content {
                role: role.clone(),
                parts: vec![Part { text: text.clone() }],
            });
        }

        let request_body = GeminiRequest {
            system_instruction: Some(SystemInstruction {
                parts: vec![Part { text: "You are an elite Multi-Sport Coach. Follow instructions precisely. The user may ask questions about the generated workout plan, or fitness. You will respond as the coach.".to_string() }]
            }),
            contents,
        };

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-3.1-pro-preview:generateContent?key={}",
            self.api_key
        );

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let err_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Gemini API error: {} - {}", status, err_text));
        }

        let gemini_response: GeminiResponse = response
            .json()
            .await
            .context("Failed to parse Gemini JSON")?;

        if let Some(error) = gemini_response.error {
            return Err(anyhow!("Gemini returned an error: {}", error.message));
        }

        if let Some(candidates) = gemini_response.candidates {
            if let Some(candidate) = candidates.first() {
                if let Some(part) = candidate.content.parts.first() {
                    return Ok(part.text.clone());
                }
            }
        }

        Err(anyhow!("No valid content returned from Gemini"))
    }

    pub fn extract_json_block(markdown: &str) -> Result<String> {
        let start_marker = "```json";
        let end_marker = "```";

        if let Some(start_idx) = markdown.find(start_marker) {
            let json_start = start_idx + start_marker.len();
            if let Some(end_idx) = markdown[json_start..].find(end_marker) {
                let json_content = &markdown[json_start..json_start + end_idx];
                return Ok(json_content.trim().to_string());
            }
        }

        // If no markers, maybe the raw string is just valid JSON
        if let Ok(_) = serde_json::from_str::<Value>(markdown) {
            return Ok(markdown.trim().to_string());
        }

        Err(anyhow!("Could not extract JSON block from LLM response"))
    }
}
