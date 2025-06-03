use jsonwebtoken::{DecodingKey, EncodingKey};
use std::{ops::Deref, sync::Arc};
pub struct ConfigInner {
    pub db_url: String,
    pub host: String,
    pub port: u16,
    pub redis_url: String,
    pub allowed_origin: String,
    pub prefork: bool,
    pub encoding_key: EncodingKey,
    pub decoding_key: DecodingKey,
}

#[derive(Clone)]
pub struct Config(Arc<ConfigInner>);

impl Config {
    pub fn from_env() -> Config {
        let v = ConfigInner {
            db_url: std::env::var("DATABASE_URL").expect("DATABASE_URL is not set in .env file"),
            host: std::env::var("HOST").expect("HOST is not set in .env file"),
            port: std::env::var("PORT")
                .expect("PORT is not set in .env file")
                .parse()
                .expect("PORT is not a number"),
            redis_url: std::env::var("REDIS_URL").expect("REDIS_URL is not set in .env file"),
            allowed_origin: std::env::var("ALLOWED_ORIGIN")
                .expect("ALLOWED_ORIGIN is not set in .env file"),
            encoding_key: EncodingKey::from_base64_secret(
                &std::env::var("JWT_ENCODING_KEY")
                    .expect("JWT_ENCODING_KEY is not set in .env file"),
            )
            .expect("Failed to create encoding key from base64 secret"),
            decoding_key: DecodingKey::from_base64_secret(
                &std::env::var("JWT_DECODING_KEY")
                    .expect("JWT_DECODING_KEY is not set in .env file"),
            )
            .expect("Failed to create decoding key from base64 secret"),
            prefork: std::env::var("PREFORK").is_ok_and(|v| v == "1"),
        };

        Self(Arc::new(v))
    }

    pub fn get_server_url(&self) -> String {
        format!("{}:{}", self.0.host, self.0.port)
    }
}

impl Deref for Config {
    type Target = ConfigInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
