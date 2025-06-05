use chrono::Utc;
use jsonwebtoken::{Header, Validation, decode, encode};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::time::Duration;

use crate::config::Config;

const JWT_EXPIRATION_DURATION: Duration = Duration::from_secs(60 * 60 * 24); // 24 hours

// Generic Claims struct
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims<T> {
    pub exp: i64,
    pub data: T,
}

pub fn encode_data<T: Serialize>(config: &Config, data: T) -> Result<String, anyhow::Error> {
    let exp = (Utc::now() + JWT_EXPIRATION_DURATION).timestamp();

    let claims = Claims { exp, data };
    encode(&Header::default(), &claims, &config.encoding_key)
        .map_err(|e| anyhow::anyhow!("Failed to encode client: {}", e))
}

pub fn decode_data<T: DeserializeOwned>(config: &Config, token: &str) -> Result<T, anyhow::Error> {
    let token_data = decode::<Claims<T>>(token, &config.decoding_key, &Validation::default())
        .map_err(|e| anyhow::anyhow!("Failed to decode client token: {}", e))?;
    Ok(token_data.claims.data)
}
