use base64::{Engine as _, engine::general_purpose};
use chrono::{Duration, Utc};
use hmac::{Hmac, Mac};
use serde::Serialize;
use sha2::Sha256;

use crate::utils::state::{CONFIG, Config};

type HmacSha256 = Hmac<Sha256>;

const PUBLIC_PATH: &str = "https://storage.sharinflame.com";

#[derive(Debug)]
pub enum Operation {
    PUT,
    DELETE,
    GET,
    HEAD,
}

impl ToString for Operation {
    fn to_string(&self) -> String {
        match self {
            Operation::PUT => "PUT".into(),
            Operation::DELETE => "DELETE".into(),
            Operation::GET => "GET".into(),
            Operation::HEAD => "HEAD".into(),
        }
    }
}

#[derive(Serialize)]
struct SignedPayload<'a> {
    expires: f64,
    allowed_operations: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    max_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    r#type: Option<&'a str>,
}

/// Normalize URL for giving it to frontend
pub fn normalize_url(url: Option<String>) -> Option<String> {
    match url {
        Some(u) if !u.contains("://") => Some(format!("{}/{}", PUBLIC_PATH, u)),
        Some(u) => Some(u.clone()),
        None => None,
    }
}

/// Private function for signing signature for tokens
fn sign(key: &[u8], msg: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).unwrap();
    mac.update(msg);
    mac.finalize().into_bytes().to_vec()
}

/// Get signed token that goes to headers and used for PUT/DELETE/HEAD/GET
pub fn generate_signed_token(
    allowed_operations: &[(Operation, &str)],
    expires_seconds: u64,
    max_size: Option<u64>,
    r#type: Option<&str>,
    config: Config,
) -> String {
    let expires_timestamp = Utc::now().timestamp() as f64 + expires_seconds as f64;

    let operations: Vec<String> = allowed_operations
        .iter()
        .map(|(op, path)| format!("{}:{}", op.to_string(), path.trim_start_matches('/')))
        .collect();

    let payload = SignedPayload {
        expires: expires_timestamp,
        allowed_operations: &operations,
        max_size,
        r#type,
    };

    let payload_json = serde_json::to_vec(&payload).unwrap();
    let payload_b64 = general_purpose::STANDARD.encode(payload_json);
    let signature = general_purpose::STANDARD.encode(sign(
        &config.cdn_secret_key.as_bytes(),
        payload_b64.as_bytes(),
    ));

    format!(
        "LV {}.{}.{}",
        &config.cdn_secret_key_n, payload_b64, signature
    )
}

/// Builds link to get object in CDN, should be used only to get an object
pub fn build_get_link(object: &str, expires_days: i64) -> String {
    let full_path = format!("{}/{}", PUBLIC_PATH, object);

    if object.starts_with("public/") {
        return full_path;
    }

    let expiry_date = (Utc::now() + Duration::days(expires_days))
        .date_naive()
        .and_hms_opt(23, 59, 59)
        .unwrap();
    let expires_timestamp = expiry_date.and_utc().timestamp();

    let payload_b64 = general_purpose::URL_SAFE_NO_PAD.encode(expires_timestamp.to_string());
    let signature_payload = format!("{}|{}", object, expires_timestamp);
    let signature = general_purpose::URL_SAFE_NO_PAD.encode(sign(
        CONFIG.cdn_secret_key.as_bytes(),
        signature_payload.as_bytes(),
    ));

    let token = format!(
        "lv.{}.{}.{}",
        CONFIG.cdn_secret_key_n, payload_b64, signature
    );

    format!("{}?token={}", full_path, token)
}

pub fn build_links(from: Vec<String>) -> Vec<String> {
    let mut new: Vec<String> = Vec::new();
    for link in from {
        new.push(build_get_link(&link, 3));
    };
    new
}
