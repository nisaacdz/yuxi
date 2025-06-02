use base64::{Engine as _, engine::general_purpose};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use std::env;
use uuid::Uuid;

use crate::error::ApiError;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,           // Subject (user ID)
    pub client_id: String,     // Client session ID
    pub user_id: Option<i32>,  // User ID from database
    pub exp: i64,              // Expiration time
    pub iat: i64,              // Issued at
    pub token_type: TokenType, // Access or Refresh
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum TokenType {
    Access,
    Refresh,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
}

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

    pub fn generate_token_pair(
        &self,
        client_id: String,
        user_id: Option<i32>,
    ) -> Result<TokenPair, ApiError> {
        let now = Utc::now();
        let access_exp = now + Duration::minutes(15); // Access token expires in 15 minutes
        let refresh_exp = now + Duration::days(7); // Refresh token expires in 7 days

        let access_claims = Claims {
            sub: client_id.clone(),
            client_id: client_id.clone(),
            user_id,
            exp: access_exp.timestamp(),
            iat: now.timestamp(),
            token_type: TokenType::Access,
        };

        let refresh_claims = Claims {
            sub: client_id,
            client_id: client_id,
            user_id,
            exp: refresh_exp.timestamp(),
            iat: now.timestamp(),
            token_type: TokenType::Refresh,
        };

        let access_token = encode(&Header::default(), &access_claims, &self.encoding_key)
            .map_err(|e| ApiError(anyhow::anyhow!("Failed to encode access token: {}", e)))?;

        let refresh_token = encode(&Header::default(), &refresh_claims, &self.encoding_key)
            .map_err(|e| ApiError(anyhow::anyhow!("Failed to encode refresh token: {}", e)))?;

        Ok(TokenPair {
            access_token,
            refresh_token,
        })
    }

    pub fn verify_token(&self, token: &str) -> Result<Claims, ApiError> {
        let token_data = decode::<Claims>(token, &self.decoding_key, &Validation::default())
            .map_err(|e| ApiError(anyhow::anyhow!("Failed to decode token: {}", e)))?;

        Ok(token_data.claims)
    }

    pub fn refresh_access_token(&self, refresh_token: &str) -> Result<String, ApiError> {
        let claims = self.verify_token(refresh_token)?;

        // Ensure this is a refresh token
        if claims.token_type != TokenType::Refresh {
            return Err(ApiError(anyhow::anyhow!("Invalid token type for refresh")));
        }

        // Generate new access token
        let now = Utc::now();
        let access_exp = now + Duration::minutes(15);

        let new_access_claims = Claims {
            sub: claims.sub,
            client_id: claims.client_id,
            user_id: claims.user_id,
            exp: access_exp.timestamp(),
            iat: now.timestamp(),
            token_type: TokenType::Access,
        };

        let access_token = encode(&Header::default(), &new_access_claims, &self.encoding_key)
            .map_err(|e| ApiError(anyhow::anyhow!("Failed to encode new access token: {}", e)))?;

        Ok(access_token)
    }
}

impl Default for JwtService {
    fn default() -> Self {
        Self::new().expect("Failed to create JwtService")
    }
}
