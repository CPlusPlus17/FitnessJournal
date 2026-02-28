use crate::models::*;
use tracing::{info, error};
use anyhow::{anyhow, Context, Result};
use reqwest::{Client, Method, RequestBuilder};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OAuth1Token {
    pub oauth_token: String,
    pub oauth_token_secret: String,
    pub mfa_token: Option<String>,
    pub mfa_expiration_timestamp: Option<String>,
    pub domain: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OAuth2Token {
    pub scope: String,
    pub jti: String,
    pub token_type: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub expires_at: Option<u64>,
    pub refresh_token_expires_in: u64,
    pub refresh_token_expires_at: Option<u64>,
}

pub struct GarminApi {
    oauth1: OAuth1Token,
    oauth2: tokio::sync::RwLock<OAuth2Token>,
    client: Client,
}

impl GarminApi {
    pub fn new() -> Result<Self> {
        let o1_str = std::fs::read_to_string("secrets/oauth1_token.json")
            .context("Failed to read secrets/oauth1_token.json. Please ensure it exists.")?;
        let oauth1: OAuth1Token =
            serde_json::from_str(&o1_str).context("Failed to parse oauth1_token.json")?;

        let o2_str = std::fs::read_to_string("secrets/oauth2_token.json")
            .context("Failed to read secrets/oauth2_token.json. Please ensure it exists.")?;
        let oauth2: OAuth2Token =
            serde_json::from_str(&o2_str).context("Failed to parse oauth2_token.json")?;

        let client = Client::builder().user_agent("GCM-iOS-5.7.2.1").build()?;

        Ok(Self {
            oauth1,
            oauth2: tokio::sync::RwLock::new(oauth2),
            client,
        })
    }

    pub fn from_oauth1_for_exchange(oauth1: OAuth1Token, client: Client) -> Result<Self> {
        let dummy_oauth2 = OAuth2Token {
            scope: String::new(),
            jti: String::new(),
            token_type: String::new(),
            access_token: String::new(),
            refresh_token: String::new(),
            expires_in: 0,
            expires_at: None,
            refresh_token_expires_in: 0,
            refresh_token_expires_at: None,
        };
        Ok(Self {
            oauth1,
            oauth2: tokio::sync::RwLock::new(dummy_oauth2),
            client,
        })
    }

    pub async fn get_oauth2_cloned(&self) -> Result<OAuth2Token> {
        Ok(self.oauth2.read().await.clone())
    }

    /// Helper to attach OAuth2 Bearer token
    async fn attach_oauth2(&self, mut req: RequestBuilder) -> RequestBuilder {
        let token = self.oauth2.read().await.access_token.clone();
        req = req.header("Authorization", format!("Bearer {}", token));
        req = req.header("DI-Backend", "connectapi.garmin.com");
        req
    }

    /// Check if the token is close to expiry
    pub async fn is_oauth2_expired(&self) -> bool {
        let oauth2 = self.oauth2.read().await;
        if let Some(expires_at) = oauth2.expires_at {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or_default();
            now >= expires_at.saturating_sub(300) // 5 minutes buffer
        } else {
            false // If no expiry tracking, cross fingers
        }
    }

    /// Refresh the OAuth2 token natively via the Garmin OAuth1 token exchange.
    pub async fn refresh_oauth2(&self) -> Result<()> {
        let consumer_key = "fc3e99d2-118c-44b8-8ae3-03370dde24c0";
        let consumer_secret = "E08WAR897WEy2knn7aFBrvegVAf0AFdWBBF";
        let url = "https://connectapi.garmin.com/oauth-service/oauth/exchange/user/2.0";

        let token = oauth1_request::Token::from_parts(
            consumer_key,
            consumer_secret,
            &self.oauth1.oauth_token,
            &self.oauth1.oauth_token_secret,
        );

        let authorization = if let Some(mfa) = &self.oauth1.mfa_token {
            let request =
                oauth1_request::ParameterList::new([("mfa_token", mfa as &dyn std::fmt::Display)]);
            oauth1_request::post(
                url,
                &request,
                &token,
                oauth1_request::signature_method::HmacSha1::new(),
            )
        } else {
            oauth1_request::post(
                url,
                &(),
                &token,
                oauth1_request::signature_method::HmacSha1::new(),
            )
        };

        let mut b = self
            .client
            .post(url)
            .header("Authorization", authorization.to_string())
            .header("Content-Type", "application/x-www-form-urlencoded");

        if let Some(mfa) = &self.oauth1.mfa_token {
            b = b.form(&[("mfa_token", mfa)]);
        }

        let res = b.send().await?;
        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Failed to refresh OAuth2 token {}: {}",
                status,
                text
            ));
        }

        let mut new_oauth2: OAuth2Token = res.json().await?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_default();
        new_oauth2.expires_at = Some(now + new_oauth2.expires_in);
        new_oauth2.refresh_token_expires_at = Some(now + new_oauth2.refresh_token_expires_in);

        let to_save = new_oauth2.clone();
        *self.oauth2.write().await = new_oauth2;

        // Save the new token locally
        std::fs::write(
            "secrets/oauth2_token.json",
            serde_json::to_string_pretty(&to_save)?,
        )?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                "secrets/oauth2_token.json",
                std::fs::Permissions::from_mode(0o600),
            )?;
        }

        info!("Successfully refreshed Garmin OAuth2 Token natively!");
        Ok(())
    }

    /// Generic connectapi GET request
    pub async fn connectapi_get(&self, endpoint: &str) -> Result<serde_json::Value> {
        let max_retries = 3;
        for attempt in 1..=max_retries {
            if self.is_oauth2_expired().await {
                self.refresh_oauth2().await?;
            }
            let url = format!("https://connectapi.garmin.com{}", endpoint);
            let mut req = self.client.request(Method::GET, &url);
            req = self.attach_oauth2(req).await;

            match req.send().await {
                Ok(res) if res.status().is_success() => {
                    return Ok(res.json().await?);
                }
                Ok(res) => {
                    let status = res.status();
                    let text = res.text().await.unwrap_or_default();
                    if attempt == max_retries {
                        return Err(anyhow!("Garmin API GET returned {}: {}", status, text));
                    }
                    tracing::warn!("Garmin API GET {} failed with {}: {}. Retrying {}/{}", endpoint, status, text, attempt, max_retries);
                }
                Err(e) => {
                    if attempt == max_retries {
                        return Err(anyhow::anyhow!("Garmin API GET request failed: {}", e));
                    }
                    tracing::warn!("Garmin API GET {} request failed: {}. Retrying {}/{}", endpoint, e, attempt, max_retries);
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(2 * attempt)).await;
        }
        unreachable!()
    }

    /// Generic connectapi POST request
    pub async fn connectapi_post(
        &self,
        endpoint: &str,
        payload: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let max_retries = 3;
        for attempt in 1..=max_retries {
            if self.is_oauth2_expired().await {
                self.refresh_oauth2().await?;
            }
            let url = format!("https://connectapi.garmin.com{}", endpoint);
            let mut req = self.client.request(Method::POST, &url);
            req = self.attach_oauth2(req).await;
            req = req.json(payload);

            match req.send().await {
                Ok(res) if res.status().is_success() => {
                    if res.status() == 204 || res.content_length() == Some(0) {
                        return Ok(serde_json::json!({}));
                    }
                    let body_text = res.text().await?;
                    if body_text.trim().is_empty() {
                        return Ok(serde_json::json!({}));
                    }
                    let json: serde_json::Value = serde_json::from_str(&body_text)?;
                    return Ok(json);
                }
                Ok(res) => {
                    let status = res.status();
                    let text = res.text().await.unwrap_or_default();
                    if attempt == max_retries {
                        return Err(anyhow!("Garmin API POST returned {}: {}", status, text));
                    }
                    tracing::warn!("Garmin API POST {} failed with {}: {}. Retrying {}/{}", endpoint, status, text, attempt, max_retries);
                }
                Err(e) => {
                    if attempt == max_retries {
                        return Err(anyhow::anyhow!("Garmin API POST request failed: {}", e));
                    }
                    tracing::warn!("Garmin API POST {} request failed: {}. Retrying {}/{}", endpoint, e, attempt, max_retries);
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(2 * attempt)).await;
        }
        unreachable!()
    }

    /// Generic connectapi DELETE request
    pub async fn connectapi_delete(&self, endpoint: &str) -> Result<()> {
        let max_retries = 3;
        for attempt in 1..=max_retries {
            if self.is_oauth2_expired().await {
                self.refresh_oauth2().await?;
            }
            let url = format!("https://connectapi.garmin.com{}", endpoint);
            let mut req = self.client.request(Method::DELETE, &url);
            req = self.attach_oauth2(req).await;

            match req.send().await {
                Ok(res) if res.status().is_success() => {
                    return Ok(());
                }
                Ok(res) => {
                    let status = res.status();
                    let text = res.text().await.unwrap_or_default();
                    if attempt == max_retries {
                        return Err(anyhow!("Garmin API DELETE returned {}: {}", status, text));
                    }
                    tracing::warn!("Garmin API DELETE {} failed with {}: {}. Retrying {}/{}", endpoint, status, text, attempt, max_retries);
                }
                Err(e) => {
                    if attempt == max_retries {
                        return Err(anyhow::anyhow!("Garmin API DELETE request failed: {}", e));
                    }
                    tracing::warn!("Garmin API DELETE {} request failed: {}. Retrying {}/{}", endpoint, e, attempt, max_retries);
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(2 * attempt)).await;
        }
        unreachable!()
    }

    pub async fn get_activities(&self, start: u32, limit: u32) -> Result<Vec<GarminActivity>> {
        let endpoint = format!(
            "/activitylist-service/activities/search/activities?start={}&limit={}",
            start, limit
        );
        let val = self.connectapi_get(&endpoint).await?;
        let activities: Vec<GarminActivity> = serde_json::from_value(val)?;
        Ok(activities)
    }

    pub async fn get_activity_exercise_sets(
        &self,
        activity_id: i64,
    ) -> Result<Option<GarminSetsData>> {
        let _endpoint = format!("/activity-service/activity/{}/duration", activity_id); // The garminconnect py package uses /activity-service/activity/{id} but 'duration' holds sets sometimes? Actually python uses get_activity_exercise_sets -> /activity-service/activity/{}/exerciseSets? Is it exerciseSets? Yes, python uses /activity-service/activity/{id} -> wait, the garminconnect package source...
                                                                                        // Let's use /activity-service/activity/{}/exerciseSets which is standard for strength
        let endpoint = format!("/activity-service/activity/{}/exerciseSets", activity_id);

        match self.connectapi_get(&endpoint).await {
            Ok(val) => {
                let sets: GarminSetsData = serde_json::from_value(val)?;
                Ok(Some(sets))
            }
            Err(e) => {
                // For non-strength activities, this might 404
                info!("Failed to get sets for activity {}: {}", activity_id, e);
                Ok(None)
            }
        }
    }

    pub async fn get_training_plans(&self) -> Result<serde_json::Value> {
        self.connectapi_get("/training-api/trainingplan/trainingplans")
            .await
    }

    pub async fn get_user_profile(&self) -> Result<serde_json::Value> {
        self.connectapi_get("/userprofile-service/socialProfile")
            .await
    }

    pub async fn get_max_metrics(&self, today_iso: &str) -> Result<serde_json::Value> {
        let endpoint = format!(
            "/metrics-service/metrics/maxmet/daily/{}/{}",
            today_iso, today_iso
        );
        self.connectapi_get(&endpoint).await
    }

    pub async fn get_calendar(
        &self,
        year: i32,
        month_zero_based: i32,
    ) -> Result<serde_json::Value> {
        let endpoint = format!("/calendar-service/year/{}/month/{}", year, month_zero_based);
        self.connectapi_get(&endpoint).await
    }

    pub async fn get_adaptive_training_plan_by_id(
        &self,
        plan_id: &str,
    ) -> Result<serde_json::Value> {
        let endpoint = format!("/training-api/trainingplan/trainingplans/{}", plan_id);
        self.connectapi_get(&endpoint).await
    }

    pub async fn get_workouts(&self) -> Result<serde_json::Value> {
        self.connectapi_get("/workout-service/workouts").await
    }

    pub async fn get_sleep_data(
        &self,
        display_name: &str,
        date_iso: &str,
    ) -> Result<serde_json::Value> {
        let endpoint = format!(
            "/wellness-service/wellness/dailySleepData/{}?date={}&nonSleepBufferMinutes=60",
            display_name, date_iso
        );
        self.connectapi_get(&endpoint).await
    }

    pub async fn get_body_battery(
        &self,
        date_iso: &str,
    ) -> std::result::Result<serde_json::Value, anyhow::Error> {
        let endpoint = format!(
            "/wellness-service/wellness/bodyBattery/reports/daily?startDate={}&endDate={}",
            date_iso, date_iso
        );
        self.connectapi_get(&endpoint).await
    }

    pub async fn get_training_readiness(
        &self,
        date_iso: &str,
    ) -> std::result::Result<serde_json::Value, anyhow::Error> {
        let endpoint = format!("/metrics-service/metrics/trainingreadiness/{}", date_iso);
        self.connectapi_get(&endpoint).await
    }

    pub async fn get_hrv_status(
        &self,
        date_iso: &str,
    ) -> std::result::Result<serde_json::Value, anyhow::Error> {
        let endpoint = format!("/hrv-service/hrv/{}", date_iso);
        self.connectapi_get(&endpoint).await
    }

    pub async fn get_rhr_trend(
        &self,
        display_name: &str,
        start_iso: &str,
        end_iso: &str,
    ) -> std::result::Result<serde_json::Value, anyhow::Error> {
        let endpoint = format!(
            "/userstats-service/wellness/daily/{}?fromDate={}&untilDate={}&metricId=60",
            display_name, start_iso, end_iso
        );
        self.connectapi_get(&endpoint).await
    }
}
