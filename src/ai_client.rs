use anyhow::{anyhow, Context, Result};
use tracing::info;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiRequest {
    system_instruction: Option<SystemInstruction>,
    contents: Vec<Content>,
    generation_config: Option<GenerationConfig>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GenerationConfig {
    max_output_tokens: i32,
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
    #[serde(rename = "usageMetadata")]
    usage_metadata: Option<UsageMetadata>,
}

#[derive(Deserialize)]
struct UsageMetadata {
    #[serde(rename = "promptTokenCount")]
    prompt_token_count: i32,
    #[serde(rename = "candidatesTokenCount")]
    candidates_token_count: Option<i32>,
    #[serde(rename = "totalTokenCount")]
    total_token_count: i32,
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
    model: String,
}

impl AiClient {
    pub fn new(api_key: String, model: String) -> Self {
        info!("Initialized AiClient with model: {}", model);
        AiClient {
            client: Client::new(),
            api_key,
            model,
        }
    }

    fn get_valid_exercises_string() -> String {
        let mut names = Vec::new();
        if let Ok(content) = std::fs::read_to_string("Garmin Exercises Database - Exercises.csv") {
            for line in content.lines().skip(2) {
                if let Some(name) = line.split(',').next() {
                    let trim = name.trim().replace("\"", "");
                    if !trim.is_empty() {
                        names.push(trim);
                    }
                }
            }
        }
        if names.is_empty() {
            "".to_string()
        } else {
            format!("\n\nCRITICAL RULE: When writing the JSON workout 'exercise' fields, YOU MUST EXACTLY MATCH one of the names from the following list. DO NOT hallucinate exercises (e.g. do not invent 'Dumbbell Goblet Squat'). If you cannot find the perfect exercise, use the closest matching exact string from this list:\n{}", names.join(", "))
        }
    }

    pub async fn generate_workout(&self, prompt: &str) -> Result<String> {
        let mut sys_text =
            "You are an elite Multi-Sport Coach. Follow instructions precisely. When creating a structured workout, incorporate supersets whenever possible. Use 'sets' and 'reps' for multiple iterations of an exercise. To represent a superset, group the multiple exercises into an 'exercises' array inside the workout step, specifying 'reps' and 'weight' for each sub-exercise, and 'sets' at the top step level.".to_string();
        sys_text.push_str(&Self::get_valid_exercises_string());

        let request_body = GeminiRequest {
            system_instruction: Some(SystemInstruction {
                parts: vec![Part { text: sys_text }],
            }),
            contents: vec![Content {
                role: "user".to_string(),
                parts: vec![Part {
                    text: prompt.to_string(),
                }],
            }],
            generation_config: Some(GenerationConfig {
                max_output_tokens: 8192,
            }),
        };

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model,
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

        if let Some(usage) = &gemini_response.usage_metadata {
            tracing::info!(
                "Gemini API Token Usage - Prompt: {}, Output: {}, Total: {}",
                usage.prompt_token_count,
                usage.candidates_token_count.unwrap_or(0),
                usage.total_token_count
            );
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

    pub async fn chat_with_history(
        &self,
        history: &[(String, String, u64)],
        context: Option<&str>,
    ) -> Result<String> {
        let mut contents = Vec::new();
        for (role, text, _) in history {
            // Map the role string to the Gemini format 'user' or 'model'
            let gemini_role = if role.to_lowercase() == "user" {
                "user"
            } else {
                "model"
            };
            contents.push(Content {
                role: gemini_role.to_string(),
                parts: vec![Part { text: text.clone() }],
            });
        }

        let mut sys_instruction = "You are an elite Multi-Sport Coach. Follow instructions precisely. The user is asking questions about the generated workout plan, their Garmin health metrics, or fitness in general. You will respond as the coach in a friendly and conversational, yet brief manner.\nIf you decide to actively add or reschedule a workout for the athlete, YOU MUST output a raw JSON codeblock starting with ```json containing an array of Garmin workout objects. Use the exact formats expected representing phase, exercise, weight, sets, reps etc.\nWhen creating a structured workout, incorporate supersets whenever possible. Use 'sets' and 'reps' for multiple iterations of an exercise. To represent a superset, group the multiple exercises into an 'exercises' array inside the workout step, specifying 'reps' and 'weight' for each sub-exercise, and 'sets' at the top step level.\nALWAYS reply with natural conversation along with the json block if adding a workout.".to_string();

        if let Some(ctx) = context {
            sys_instruction.push_str("\n\n=== LIVE ATHLETE CONTEXT ===\n");
            sys_instruction.push_str(ctx);
        }

        sys_instruction.push_str(&Self::get_valid_exercises_string());

        let request_body = GeminiRequest {
            system_instruction: Some(SystemInstruction {
                parts: vec![Part {
                    text: sys_instruction,
                }],
            }),
            contents,
            generation_config: Some(GenerationConfig {
                max_output_tokens: 8192,
            }),
        };

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model,
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

        if let Some(usage) = &gemini_response.usage_metadata {
            tracing::info!(
                "Gemini API Token Usage - Prompt: {}, Output: {}, Total: {}",
                usage.prompt_token_count,
                usage.candidates_token_count.unwrap_or(0),
                usage.total_token_count
            );
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
        if serde_json::from_str::<Value>(markdown).is_ok() {
            return Ok(markdown.trim().to_string());
        }

        Err(anyhow!("Could not extract JSON block from LLM response"))
    }
}

#[cfg(test)]
mod tests {
    use super::AiClient;

    #[test]
    fn extract_json_block_from_markdown() {
        let markdown = "Here is your plan:\n```json\n[{\"workoutName\":\"FJ-AI:Test\"}]\n```";
        let extracted = AiClient::extract_json_block(markdown).expect("json block should parse");
        assert_eq!(extracted, "[{\"workoutName\":\"FJ-AI:Test\"}]");
    }

    #[test]
    fn extract_json_block_from_raw_json() {
        let raw = "{\"ok\":true}";
        let extracted = AiClient::extract_json_block(raw).expect("raw json should parse");
        assert_eq!(extracted, "{\"ok\":true}");
    }

    #[test]
    fn extract_json_block_rejects_invalid_payload() {
        let invalid = "not json";
        assert!(AiClient::extract_json_block(invalid).is_err());
    }
}
