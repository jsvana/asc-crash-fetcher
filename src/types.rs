//! App Store Connect API v4.2 response types — TestFlight Feedback subset.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ─── JSON:API pagination ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct PagedLinks {
    pub next: Option<String>,
}

// ─── Apps ────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AppsResponse {
    pub data: Vec<App>,
}

#[derive(Debug, Deserialize)]
pub struct App {
    pub id: String,
    pub attributes: Option<AppAttributes>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppAttributes {
    pub bundle_id: Option<String>,
    pub name: Option<String>,
}

// ─── BetaFeedbackCrashSubmission ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CrashSubmissionsResponse {
    pub data: Vec<CrashSubmission>,
    pub links: PagedLinks,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CrashSubmission {
    pub id: String,
    pub attributes: Option<CrashSubmissionAttrs>,
    pub relationships: Option<CrashSubmissionRels>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CrashSubmissionAttrs {
    pub created_date: Option<DateTime<Utc>>,
    pub comment: Option<String>,
    pub email: Option<String>,
    pub device_model: Option<String>,
    pub os_version: Option<String>,
    pub locale: Option<String>,
    pub time_zone: Option<String>,
    pub architecture: Option<String>,
    pub connection_type: Option<String>,
    pub app_uptime_in_milliseconds: Option<i64>,
    pub disk_bytes_available: Option<i64>,
    pub disk_bytes_total: Option<i64>,
    pub battery_percentage: Option<i32>,
    pub screen_width_in_points: Option<i32>,
    pub screen_height_in_points: Option<i32>,
    pub app_platform: Option<String>,
    pub device_platform: Option<String>,
    pub device_family: Option<String>,
    pub build_bundle_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CrashSubmissionRels {
    pub build: Option<RelData>,
    pub tester: Option<RelData>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RelData {
    pub data: Option<ResourceId>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ResourceId {
    pub id: String,
}

// ─── BetaCrashLog ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CrashLogResponse {
    pub data: CrashLog,
}

#[derive(Debug, Deserialize)]
pub struct CrashLog {
    pub attributes: Option<CrashLogAttrs>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashLogAttrs {
    pub log_text: Option<String>,
}

// ─── BetaFeedbackScreenshotSubmission ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ScreenshotSubmissionsResponse {
    pub data: Vec<ScreenshotSubmission>,
    pub links: PagedLinks,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ScreenshotSubmission {
    pub id: String,
    pub attributes: Option<ScreenshotSubmissionAttrs>,
    pub relationships: Option<ScreenshotSubmissionRels>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotSubmissionAttrs {
    pub created_date: Option<DateTime<Utc>>,
    pub comment: Option<String>,
    pub email: Option<String>,
    pub device_model: Option<String>,
    pub os_version: Option<String>,
    pub locale: Option<String>,
    pub time_zone: Option<String>,
    pub connection_type: Option<String>,
    pub battery_percentage: Option<i32>,
    pub app_platform: Option<String>,
    pub device_platform: Option<String>,
    pub device_family: Option<String>,
    pub build_bundle_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ScreenshotSubmissionRels {
    pub build: Option<RelData>,
    pub tester: Option<RelData>,
}
