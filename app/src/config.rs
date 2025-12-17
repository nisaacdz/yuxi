use jsonwebtoken::{DecodingKey, EncodingKey};
use lettre::{AsyncSmtpTransport, Tokio1Executor, transport::smtp::authentication::Credentials};
use openidconnect::{
    Client, ClientId, ClientSecret, EmptyAdditionalClaims, EndpointMaybeSet, EndpointNotSet,
    EndpointSet, IssuerUrl, RedirectUrl, StandardErrorResponse,
    core::{
        CoreAuthDisplay, CoreAuthPrompt, CoreClient, CoreErrorResponseType, CoreGenderClaim,
        CoreJsonWebKey, CoreJweContentEncryptionAlgorithm, CoreProviderMetadata,
        CoreRevocableToken, CoreRevocationErrorResponse, CoreTokenIntrospectionResponse,
        CoreTokenResponse,
    },
};

type AuthClient = Client<
    EmptyAdditionalClaims,
    CoreAuthDisplay,
    CoreGenderClaim,
    CoreJweContentEncryptionAlgorithm,
    CoreJsonWebKey,
    CoreAuthPrompt,
    StandardErrorResponse<CoreErrorResponseType>,
    CoreTokenResponse,
    CoreTokenIntrospectionResponse,
    CoreRevocableToken,
    CoreRevocationErrorResponse,
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

use std::{ops::Deref, sync::Arc};
pub struct ConfigInner {
    pub db_url: String,
    pub host: String,
    pub port: u16,
    pub allowed_origins: Vec<String>,
    // pub prefork: bool,
    pub encoding_key: EncodingKey,
    pub decoding_key: DecodingKey,
    pub emailer: String,
    pub transponder: AsyncSmtpTransport<Tokio1Executor>,
    pub google_auth_client: AuthClient,
    //pub facebook_auth_client: AuthClient,
    pub http_client: openidconnect::reqwest::Client,
}

#[derive(Clone)]
pub struct Config(Arc<ConfigInner>);

impl Config {
    pub async fn from_env() -> Config {
        #[cfg(debug_assertions)]
        dotenvy::dotenv().ok();

        let http_client = openidconnect::reqwest::Client::builder()
            .redirect(openidconnect::reqwest::redirect::Policy::none())
            .build()
            .expect("Failed to build HTTP client");

        let google_auth_client = {
            let google_client_id =
                std::env::var("GOOGLE_CLIENT_ID").expect("GOOGLE_CLIENT_ID is required");
            let google_client_secret =
                std::env::var("GOOGLE_CLIENT_SECRET").expect("GOOGLE_CLIENT_SECRET is required");
            let google_redirect_url =
                std::env::var("GOOGLE_REDIRECT_URL").expect("GOOGLE_REDIRECT_URL is required");

            let provider_metadata = CoreProviderMetadata::discover_async(
                IssuerUrl::new("https://accounts.google.com".to_string()).unwrap(),
                &http_client,
            )
            .await
            .expect("Failed to discover provider metadata");

            CoreClient::from_provider_metadata(
                provider_metadata,
                ClientId::new(google_client_id),
                Some(ClientSecret::new(google_client_secret)),
            )
            .set_redirect_uri(RedirectUrl::new(google_redirect_url).expect("Invalid redirect URL"))
        };

        // let facebook_auth_client = {
        //     let facebook_app_id =
        //         std::env::var("FACEBOOK_APP_ID").expect("FACEBOOK_APP_ID is required");
        //     let facebook_app_secret =
        //         std::env::var("FACEBOOK_APP_SECRET").expect("FACEBOOK_APP_SECRET is required");
        //     let facebook_redirect_url =
        //         std::env::var("FACEBOOK_REDIRECT_URL").expect("FACEBOOK_REDIRECT_URL is required");

        //     let provider_metadata = CoreProviderMetadata::discover_async(
        //         IssuerUrl::new("https://www.facebook.com".to_string()).unwrap(),
        //         &http_client,
        //     )
        //     .await
        //     .expect("Failed to discover Facebook provider metadata");

        //     CoreClient::from_provider_metadata(
        //         provider_metadata,
        //         ClientId::new(facebook_app_id),
        //         Some(ClientSecret::new(facebook_app_secret)),
        //     )
        //     .set_redirect_uri(
        //         RedirectUrl::new(facebook_redirect_url).expect("Invalid redirect URL"),
        //     )
        // };

        let v = ConfigInner {
            db_url: std::env::var("DATABASE_URL").expect("DATABASE_URL is required"),
            host: std::env::var("HOST").expect("HOST is required"),
            port: std::env::var("PORT")
                .expect("PORT is required")
                .parse()
                .expect("PORT is not a number"),
            allowed_origins: std::env::var("ALLOWED_ORIGINS").expect("ALLOWED_ORIGIN is required").split(",").map(ToOwned::to_owned).collect::<Vec<_>>(),
            encoding_key: EncodingKey::from_secret(
                std::env::var("JWT_SECRET")
                    .expect("JWT_SECRET is required")
                    .as_bytes(),
            ),
            decoding_key: DecodingKey::from_secret(
                std::env::var("JWT_SECRET")
                    .expect("JWT_SECRET is required")
                    .as_bytes(),
            ),
            emailer: std::env::var("EMAILER").expect("EMAILER is required"),
            transponder: AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(
                &std::env::var("SMTP_HOST").expect("SMTP_HOST is required"),
            )
            .expect("Failed to create SMTP transport")
            .port(
                std::env::var("SMTP_PORT")
                    .expect("SMTP_PORT is required")
                    .parse()
                    .expect("SMTP_PORT is not a number"),
            )
            .credentials(Credentials::new(
                std::env::var("SMTP_USER").expect("SMTP_USER is required"),
                std::env::var("SMTP_PASS").expect("SMTP_PASS is required"),
            ))
            .build(),
            google_auth_client,
            //facebook_auth_client,
            http_client,
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
