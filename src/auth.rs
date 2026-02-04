//! ES256 JWT generation for App Store Connect API authentication.

use anyhow::{Context, Result};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize)]
struct Claims {
    iss: String,
    iat: u64,
    exp: u64,
    aud: String,
}

/// Generate a short-lived ES256 JWT for the App Store Connect API.
pub fn generate_token(issuer_id: &str, key_id: &str, private_key: &str) -> Result<String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("clock error")?
        .as_secs();

    let claims = Claims {
        iss: issuer_id.to_string(),
        iat: now,
        exp: now + 20 * 60,
        aud: "appstoreconnect-v1".to_string(),
    };

    let mut header = Header::new(Algorithm::ES256);
    header.kid = Some(key_id.to_string());
    header.typ = Some("JWT".to_string());

    let key = EncodingKey::from_ec_pem(private_key.as_bytes())
        .context("failed to parse .p8 private key")?;

    encode(&header, &claims, &key).context("failed to encode JWT")
}
