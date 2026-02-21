use crate::garmin_api::{GarminApi, OAuth1Token, OAuth2Token};
use anyhow::{anyhow, Context, Result};
use lazy_static::lazy_static;
use regex::Regex;
use reqwest::Client;
use std::collections::HashMap;

lazy_static! {
    static ref CSRF_RE: Regex = Regex::new(r#"name="_csrf"\s+value="(.+?)""#).unwrap();
    static ref TITLE_RE: Regex = Regex::new(r#"<title>(.+?)</title>"#).unwrap();
    static ref TICKET_RE: Regex = Regex::new(r#"embed\?ticket=([^"]+)""#).unwrap();
}

const SSO_DOMAIN: &str = "garmin.com";
const USER_AGENT: &str = "com.garmin.android.apps.connectmobile";
const CONSUMER_KEY: &str = "fc3e99d2-118c-44b8-8ae3-03370dde24c0";
const CONSUMER_SECRET: &str = "E08WAR897WEy2knn7aFBrvegVAf0AFdWBBF";

pub struct GarminLoginSession {
    pub client: Client,
    pub signin_params: HashMap<&'static str, &'static str>,
}

pub enum LoginResult {
    Success(OAuth1Token, Box<OAuth2Token>),
    MfaRequired(GarminLoginSession),
}

impl GarminLoginSession {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .cookie_store(true)
            .build()?;

        let mut signin_params = HashMap::new();
        signin_params.insert("id", "gauth-widget");
        signin_params.insert("embedWidget", "true");
        signin_params.insert("gauthHost", "https://sso.garmin.com/sso");
        signin_params.insert("service", "https://sso.garmin.com/sso/embed");
        signin_params.insert("source", "https://sso.garmin.com/sso/embed");
        signin_params.insert(
            "redirectAfterAccountLoginUrl",
            "https://sso.garmin.com/sso/embed",
        );
        signin_params.insert(
            "redirectAfterAccountCreationUrl",
            "https://sso.garmin.com/sso/embed",
        );

        Ok(Self {
            client,
            signin_params,
        })
    }
}

pub async fn login_step_1(email: &str, password: &str) -> Result<LoginResult> {
    let session = GarminLoginSession::new()?;
    let client = &session.client;

    // 1. Initial embed load
    let mut embed_params = HashMap::new();
    embed_params.insert("id", "gauth-widget");
    embed_params.insert("embedWidget", "true");
    embed_params.insert("gauthHost", "https://sso.garmin.com/sso");

    let embed_url = "https://sso.garmin.com/sso/embed";
    client
        .get(embed_url)
        .query(&embed_params)
        .send()
        .await?
        .error_for_status()?;

    // 2. Load signin to get CSRF
    let signin_url = "https://sso.garmin.com/sso/signin";
    let signin_resp = client
        .get(signin_url)
        .query(&session.signin_params)
        .header("referer", embed_url)
        .send()
        .await?
        .error_for_status()?;

    let signin_html = signin_resp.text().await?;
    let csrf_match = CSRF_RE
        .captures(&signin_html)
        .context("Could not find CSRF token on Garmin login page")?;
    let csrf_token = csrf_match.get(1).unwrap().as_str();

    // 3. Post credentials
    let mut form_data = HashMap::new();
    form_data.insert("username", email);
    form_data.insert("password", password);
    form_data.insert("embed", "true");
    form_data.insert("_csrf", csrf_token);

    let login_resp = client
        .post(signin_url)
        .query(&session.signin_params)
        .header("referer", signin_url)
        .form(&form_data)
        .send()
        .await?
        .error_for_status()?;

    let result_html = login_resp.text().await?;

    let title_match = TITLE_RE
        .captures(&result_html)
        .context("Could not find title on Garmin result page")?;
    let title = title_match.get(1).unwrap().as_str();

    if title.contains("MFA") {
        return Ok(LoginResult::MfaRequired(session));
    } else if title != "Success" {
        return Err(anyhow!("Unexpected login result title: {}", title));
    }

    // Success -> parse ticket and finish
    let ticket = extract_ticket(&result_html)?;
    complete_login(session.client, ticket)
        .await
        .map(|(o1, o2)| LoginResult::Success(o1, Box::new(o2)))
}

pub async fn login_step_2_mfa(
    session: GarminLoginSession,
    mfa_code: &str,
) -> Result<(OAuth1Token, OAuth2Token)> {
    let client = session.client;

    // To handle MFA we need a new CSRF from the current screen (which was a redirect after login post)
    // Unfortunately we didn't save the HTML. We'll execute an empty get to current page or
    // just try a direct MFA post (which requires a fresh CSRF from wherever we landed).
    // The easiest robust way is to fetch the current MFA form. Let's just rely on a new hit to signin.
    // Wait, the cookies are set. Let's hit the embed page again or we can re-extract from the MFA response.
    // Actually we can just GET the MFA page to get a CSRF token.
    // In garth: client.last_resp.text contains the MFA form with the CSRF.
    // We can do another GET to embedded signin or just retry the signin endpoint to yield the MFA screen again safely.
    let signin_url = "https://sso.garmin.com/sso/signin";
    let mfa_page_resp = client
        .get(signin_url)
        .query(&session.signin_params)
        .send()
        .await?
        .error_for_status()?;
    let mfa_html = mfa_page_resp.text().await?;
    let csrf_match = CSRF_RE
        .captures(&mfa_html)
        .context("Could not find CSRF token on MFA page")?;
    let csrf_token = csrf_match.get(1).unwrap().as_str();

    let mut form_data = HashMap::new();
    form_data.insert("mfa-code", mfa_code);
    form_data.insert("embed", "true");
    form_data.insert("_csrf", csrf_token);
    form_data.insert("fromPage", "setupEnterMfaCode");

    let verify_url = "https://sso.garmin.com/sso/verifyMFA/loginEnterMfaCode";
    let verify_resp = client
        .post(verify_url)
        .query(&session.signin_params)
        .header("referer", signin_url)
        .form(&form_data)
        .send()
        .await?
        .error_for_status()?;

    let result_html = verify_resp.text().await?;
    let title_match = TITLE_RE
        .captures(&result_html)
        .context("Could not find title after MFA verification")?;
    let title = title_match.get(1).unwrap().as_str();

    if title != "Success" {
        return Err(anyhow!("MFA verification failed. Title: {}", title));
    }

    let ticket = extract_ticket(&result_html)?;
    complete_login(client, ticket).await
}

fn extract_ticket(html: &str) -> Result<String> {
    let ticket_match = TICKET_RE
        .captures(html)
        .context("Could not find ticket in response HTML")?;
    Ok(ticket_match.get(1).unwrap().as_str().to_string())
}

async fn complete_login(client: Client, ticket: String) -> Result<(OAuth1Token, OAuth2Token)> {
    // 1. Get OAuth1
    let base_url = "https://connectapi.garmin.com/oauth-service/oauth/preauthorized";
    let login_url = "https://sso.garmin.com/sso/embed";

    let request_params = oauth1_request::ParameterList::new([
        ("ticket", ticket.as_str()),
        ("login-url", login_url),
        ("accepts-mfa-tokens", "true"),
    ]);

    // We must sign this GET request using the Consumer Key/Secret (and NO token).
    let builder: oauth1_request::Builder<
        '_,
        oauth1_request::signature_method::HmacSha1,
        &str,
        &str,
    > = oauth1_request::Builder::new(
        oauth1_request::Credentials::new(CONSUMER_KEY, CONSUMER_SECRET),
        oauth1_request::signature_method::HmacSha1::new(),
    );
    let authorization = builder.authorize("GET", base_url, &request_params);

    let o1_resp = client
        .get(base_url)
        .query(&[
            ("ticket", ticket.as_str()),
            ("login-url", login_url),
            ("accepts-mfa-tokens", "true"),
        ])
        .header("Authorization", authorization)
        .header("User-Agent", USER_AGENT)
        .send()
        .await?
        .error_for_status()?;

    let o1_text = o1_resp.text().await?;
    let parsed_qs: HashMap<String, String> = url::form_urlencoded::parse(o1_text.as_bytes())
        .into_owned()
        .collect();

    let oauth_token = parsed_qs
        .get("oauth_token")
        .context("Missing oauth_token")?
        .clone();
    let oauth_token_secret = parsed_qs
        .get("oauth_token_secret")
        .context("Missing oauth_token_secret")?
        .clone();
    let mfa_token = parsed_qs.get("mfa_token").cloned();
    let mfa_expiration_timestamp = parsed_qs.get("mfa_expiration_timestamp").cloned();

    let oauth1 = OAuth1Token {
        oauth_token,
        oauth_token_secret,
        mfa_token,
        mfa_expiration_timestamp,
        domain: SSO_DOMAIN.to_string(),
    };

    // 2. Exchange for OAuth2
    let api_mock = GarminApi::from_oauth1_for_exchange(oauth1.clone(), client)?;
    api_mock.refresh_oauth2().await?;

    let final_oauth2 = api_mock.get_oauth2_cloned().await?;

    Ok((oauth1, final_oauth2))
}
