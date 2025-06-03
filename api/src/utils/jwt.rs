use base64::{Engine as _, engine::general_purpose};
use chrono::Utc;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use models::schemas::user::ClientSchema;
use serde::{Deserialize, Serialize};
use std::env;

use crate::error::ApiError;

pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

impl JwtService {
    pub fn new() -> Result<Self, ApiError> {
        let secret = env::var("JWT_SECRET").unwrap_or_else(|_| "your-secret-key".to_string());
        let key = general_purpose::STANDARD.encode(secret.as_bytes());
        let encoding_key = EncodingKey::from_base64_secret(&key)
            .map_err(|e| ApiError(anyhow::anyhow!("Failed to create encoding key: {}", e)))?;
        let decoding_key = DecodingKey::from_base64_secret(&key)
            .map_err(|e| ApiError(anyhow::anyhow!("Failed to create decoding key: {}", e)))?;
        Ok(Self {
            encoding_key,
            decoding_key,
        })
    }

    pub fn encode_client(&self, client: &ClientSchema) -> Result<String, ApiError> {
        encode(&Header::default(), client, &self.encoding_key)
            .map_err(|e| ApiError(anyhow::anyhow!("Failed to encode client: {}", e)))
    }

    pub fn decode_client(&self, token: &str) -> Result<ClientSchema, ApiError> {
        let token_data = decode::<ClientSchema>(token, &self.decoding_key, &Validation::default())
            .map_err(|e| ApiError(anyhow::anyhow!("Failed to decode client token: {}", e)))?;
        Ok(token_data.claims)
    }
}

impl Default for JwtService {
    fn default() -> Self {
        Self::new().expect("Failed to create JwtService")
    }
}
