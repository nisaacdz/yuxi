use jsonwebtoken::{DecodingKey, EncodingKey};
use std::{ops::Deref, sync::Arc};
pub struct ConfigInner {
    pub db_url: String,
    pub host: String,
    pub port: u16,
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
            allowed_origin: std::env::var("ALLOWED_ORIGIN")
                .expect("ALLOWED_ORIGIN is not set in .env file"),
            encoding_key: EncodingKey::from_secret(
                std::env::var("JWT_SECRET")
                    .expect("JWT_SECRET is not set in .env file")
                    .as_bytes(),
            ),
            decoding_key: DecodingKey::from_secret(
                std::env::var("JWT_SECRET")
                    .expect("JWT_SECRET is not set in .env file")
                    .as_bytes(),
            ),
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
