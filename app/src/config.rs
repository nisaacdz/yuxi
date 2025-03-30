pub struct Config {
    pub db_url: String,
    pub host: String,
    pub port: u32,
    pub redis_url: String,
    pub allowed_origin: String,
    pub allowed_origin_header: String,
    pub prefork: bool,
}

impl Config {
    pub fn from_env() -> Config {
        Config {
            db_url: std::env::var("DATABASE_URL").expect("DATABASE_URL is not set in .env file"),
            host: std::env::var("HOST").expect("HOST is not set in .env file"),
            port: std::env::var("PORT")
                .expect("PORT is not set in .env file")
                .parse()
                .expect("PORT is not a number"),
            redis_url: std::env::var("REDIS_URL").expect("REDIS_URL is not set in .env file"),
            allowed_origin: std::env::var("ALLOWED_ORIGIN")
                .expect("ALLOWED_ORIGIN is not set in .env file"),
            allowed_origin_header: std::env::var("ALLOWED_ORIGIN_HEADER")
                .expect("ALLOWED_ORIGIN_HEADER is not set in .env file"),
            prefork: std::env::var("PREFORK").is_ok_and(|v| v == "1"),
        }
    }

    pub fn get_server_url(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
