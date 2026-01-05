use base64::Engine;
use base64::engine::general_purpose;
use hmac::Hmac;
use hmac::Mac;
use pbkdf2::pbkdf2_hmac;
use rand::TryRngCore;
use rand::rngs::OsRng;
use serde::Serialize;
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::utils::state::CONFIG;

type HmacSha256 = Hmac<Sha256>;

pub fn b64_encode(data: &[u8]) -> String {
    general_purpose::STANDARD.encode(data)
}

pub fn b64_decode(s: &str) -> Result<Vec<u8>, base64::DecodeError> {
    general_purpose::STANDARD.decode(s)
}

pub fn generate_key(length: usize) -> String {
    let mut bytes = vec![0u8; length];
    OsRng.try_fill_bytes(&mut bytes).unwrap();
    b64_encode(&bytes)
}

pub fn generate_salt() -> [u8; 16] {
    let mut salt = [0u8; 16];
    OsRng.try_fill_bytes(&mut salt).unwrap();
    salt
}

pub fn hash_password(password: &str, salt: &[u8]) -> Vec<u8> {
    let mut hash = vec![0u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, 10_000, &mut hash);
    hash
}

pub fn store_password(password: &str) -> String {
    let salt = generate_salt();
    let hashed = hash_password(password, &salt);
    format!("{}${}", hex::encode(salt), hex::encode(hashed))
}

pub fn check_password(stored: &str, password: &str) -> bool {
    let parts: Vec<&str> = stored.split('$').collect();
    if parts.len() != 2 {
        return false;
    }
    let salt = hex::decode(parts[0]).unwrap();
    let stored_hash = hex::decode(parts[1]).unwrap();
    let new_hash = hash_password(password, &salt);
    new_hash == stored_hash
}

pub async fn store_password_async(password: String) -> String {
    tokio::task::spawn_blocking(move || store_password(&password))
        .await
        .expect("blocking task panicked")
}

pub async fn check_password_async(stored: String, password: String) -> bool {
    tokio::task::spawn_blocking(move || check_password(&stored, &password))
        .await
        .expect("blocking task panicked")
}

#[derive(Debug, Serialize)]
pub struct DecodedToken {
    pub user_id: String,
    pub is_expired: bool,
    pub expiration_timestamp: u64,
    pub secret: String,
    pub key_type: String,
    pub session_id: String,
}

fn hmac_sha256_b64(message: &str, signature_key: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(signature_key.as_bytes()).unwrap();
    mac.update(message.as_bytes());
    let result = mac.finalize().into_bytes();
    general_purpose::STANDARD.encode(result)
}

fn verify_hmac_b64(message: &str, sig_b64: &str, signature_key: &str) -> bool {
    let expected = hmac_sha256_b64(message, signature_key);
    expected.eq(sig_b64)
}

pub async fn generate_token(
    user_id: &str,
    key_type: &str,
    long_term: bool,
    secret: &str,
    session_id: &str,
) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let expiration = if long_term {
        now + 30 * 24 * 3600
    } else {
        now + 3600
    };

    let combined = format!(
        "{}\0{}\0{}\0{}\0{}",
        user_id, expiration, secret, session_id, key_type
    );

    let payload = b64_encode(&combined.as_bytes());

    let signature = hmac_sha256_b64(&payload, &CONFIG.signature_key);

    let token = format!("LV {}.{}", payload, signature);
    token
}

pub fn decode_token(
    token: &str,
    verify_type: Option<&'static str>,
) -> Result<DecodedToken, &'static str> {
    if !token.starts_with("LV ") {
        return Err("INVALID_TOKEN");
    }

    // remove prefix
    let t = &token[3..];

    // split last '.' for signature
    let parts_rev: Vec<&str> = t.rsplitn(2, '.').collect();
    if parts_rev.len() != 2 {
        return Err("INVALID_TOKEN_FORMAT");
    }
    // rsplitn produced [signature, payload]
    let signature = parts_rev[0];
    let payload = parts_rev[1];

    if !verify_hmac_b64(&payload, signature, &CONFIG.signature_key) {
        return Err("INVALID_SIGNATURE");
    }

    // decrypt
    let decrypted = match b64_decode(payload) {
        Ok(b) => b,
        Err(_) => {
            return Err("DECODE_ERROR");
        }
    };

    let decoded_str = match String::from_utf8(decrypted) {
        Ok(s) => s,
        Err(_) => {
            return Err("DECODE_ERROR");
        }
    };

    let parts: Vec<&str> = decoded_str.split('\0').collect();
    if parts.len() != 5 {
        return Err("DECODE_ERROR");
    }

    let user_id = parts[0].to_string();
    let expiration_ts = match parts[1].parse::<u64>() {
        Ok(v) => v,
        Err(_) => {
            return Err("DECODE_ERROR");
        }
    };
    let secret = parts[2].to_string();
    let session_id = parts[3].to_string();
    let key_type = parts[4].to_string();

    if verify_type.is_some() && verify_type != Some(&key_type) {
        return Err("INVALID_TOKEN");
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let is_expired = now > expiration_ts;

    Ok(DecodedToken {
        user_id: user_id,
        is_expired: is_expired,
        expiration_timestamp: expiration_ts,
        secret: secret,
        session_id: session_id,
        key_type: key_type,
    })
}
