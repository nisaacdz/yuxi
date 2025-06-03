use jsonwebtoken::{Header, Validation, decode, encode};
use serde::{Serialize, de::DeserializeOwned};

use crate::config::Config;

pub fn encode_data<T: Serialize>(config: &Config, data: T) -> Result<String, anyhow::Error> {
    encode(&Header::default(), &data, &config.encoding_key)
        .map_err(|e| anyhow::anyhow!("Failed to encode client: {}", e))
}

pub fn decode_data<T: DeserializeOwned>(config: &Config, token: &str) -> Result<T, anyhow::Error> {
    let token_data = decode::<T>(token, &config.decoding_key, &Validation::default())
        .map_err(|e| anyhow::anyhow!("Failed to decode client token: {}", e))?;
    Ok(token_data.claims)
}
