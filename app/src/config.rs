use jsonwebtoken::{DecodingKey, EncodingKey};
use lettre::{AsyncSmtpTransport, Tokio1Executor, transport::smtp::authentication::Credentials};
use std::{ops::Deref, sync::Arc};
pub struct ConfigInner {
    pub db_url: String,
    pub host: String,
    pub port: u16,
    pub allowed_origin: String,
    // pub prefork: bool,
    pub encoding_key: EncodingKey,
    pub decoding_key: DecodingKey,
    pub emailer: String,
    pub transponder: AsyncSmtpTransport<Tokio1Executor>,
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
            emailer: std::env::var("EMAILER").expect("EMAILER is not set in .env file"),
            transponder: AsyncSmtpTransport::<Tokio1Executor>::relay(
                &std::env::var("SMTP_HOST").expect("SMTP_HOST is not set in .env file"),
            )
            .expect("Failed to create SMTP transport")
            .port(
                std::env::var("SMTP_PORT")
                    .expect("SMTP_PORT is not set in .env file")
                    .parse()
                    .expect("SMTP_PORT is not a number"),
            )
            .credentials(Credentials::new(
                std::env::var("SMTP_USER").expect("SMTP_USER is not set in .env file"),
                std::env::var("SMTP_PASS").expect("SMTP_PASS is not set in .env file"),
            ))
            .build(),
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
