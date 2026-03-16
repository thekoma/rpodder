use serde::Deserialize;

/// Application configuration, loaded from env vars and optional config file.
#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    #[serde(default = "default_database_url")]
    pub database_url: String,

    #[serde(default = "default_host")]
    pub host: String,

    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default = "default_migrations_dir")]
    pub migrations_dir: String,

    #[serde(default)]
    pub run_migrations: bool,
}

fn default_database_url() -> String {
    "sqlite://rpodder.db".into()
}

fn default_host() -> String {
    "127.0.0.1".into()
}

fn default_port() -> u16 {
    3005
}

fn default_migrations_dir() -> String {
    "migrations".into()
}

impl AppConfig {
    /// Load config from environment variables (RPODDER_*) and optional config file.
    pub fn load(config_file: Option<&str>) -> anyhow::Result<Self> {
        let mut builder = config::Config::builder();

        // Load from config file if provided
        if let Some(path) = config_file {
            builder = builder.add_source(config::File::with_name(path).required(false));
        }

        // Environment variables override file settings: RPODDER_DATABASE_URL, RPODDER_HOST, etc.
        builder = builder.add_source(
            config::Environment::with_prefix("RPODDER")
                .prefix_separator("_")
                .try_parsing(true),
        );

        let config = builder.build()?;
        Ok(config.try_deserialize()?)
    }
}
