//! HTTP client for the App Store Connect API v4.2.

use anyhow::{bail, Context, Result};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use tracing::debug;

use crate::auth;
use crate::types::*;

const BASE: &str = "https://api.appstoreconnect.apple.com";

pub struct AscClient {
    http: reqwest::Client,
    issuer_id: String,
    key_id: String,
    private_key: String,
}

impl AscClient {
    pub fn new(issuer_id: String, key_id: String, private_key: String) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent("asc-crash-fetcher/0.2.0")
            .build()?;
        Ok(Self {
            http,
            issuer_id,
            key_id,
            private_key,
        })
    }

    fn token(&self) -> Result<String> {
        auth::generate_token(&self.issuer_id, &self.key_id, &self.private_key)
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        let token = self.token()?;
        debug!(url, "GET");
        let resp = self
            .http
            .get(url)
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .header(CONTENT_TYPE, "application/json")
            .send()
            .await
            .context("request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            bail!("API {status}: {body}");
        }
        resp.json().await.context("parse response")
    }

    /// GET that returns None on 404 (for optional endpoints like crash logs).
    async fn get_optional<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<Option<T>> {
        let token = self.token()?;
        debug!(url, "GET (optional)");
        let resp = self
            .http
            .get(url)
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .header(CONTENT_TYPE, "application/json")
            .send()
            .await
            .context("request failed")?;

        if resp.status().as_u16() == 404 {
            return Ok(None);
        }
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            bail!("API {status}: {body}");
        }
        let val: T = resp.json().await?;
        Ok(Some(val))
    }

    // ─── Apps ────────────────────────────────────────────────────────────

    pub async fn list_apps(&self) -> Result<Vec<App>> {
        let r: AppsResponse = self
            .get_json(&format!("{BASE}/v1/apps?fields[apps]=name,bundleId"))
            .await?;
        Ok(r.data)
    }

    pub async fn find_app(&self, bundle_id: &str) -> Result<Option<App>> {
        let r: AppsResponse = self
            .get_json(&format!(
                "{BASE}/v1/apps?filter[bundleId]={bundle_id}&fields[apps]=name,bundleId"
            ))
            .await?;
        Ok(r.data.into_iter().next())
    }

    // ─── Crash submissions ───────────────────────────────────────────────

    pub async fn get_crash_page(&self, url: &str) -> Result<CrashSubmissionsResponse> {
        self.get_json(url).await
    }

    /// Build the initial URL for listing crash submissions.
    pub fn crash_list_url(app_asc_id: &str) -> String {
        format!(
            "{BASE}/v1/apps/{app_asc_id}/betaFeedbackCrashSubmissions\
             ?fields[betaFeedbackCrashSubmissions]=\
             createdDate,comment,email,deviceModel,osVersion,locale,\
             timeZone,architecture,connectionType,appUptimeInMilliseconds,\
             diskBytesAvailable,diskBytesTotal,batteryPercentage,\
             screenWidthInPoints,screenHeightInPoints,appPlatform,\
             devicePlatform,deviceFamily,buildBundleId\
             &sort=-createdDate\
             &limit=200"
        )
    }

    /// Download the crash log text for a submission. Returns None if not yet available.
    pub async fn get_crash_log(&self, submission_id: &str) -> Result<Option<String>> {
        let url = format!(
            "{BASE}/v1/betaFeedbackCrashSubmissions/{submission_id}/crashLog\
             ?fields[betaCrashLogs]=logText"
        );
        let resp: Option<CrashLogResponse> = self.get_optional(&url).await?;
        Ok(resp
            .and_then(|r| r.data.attributes)
            .and_then(|a| a.log_text))
    }
}
