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

    #[serde(default = "default_true")]
    pub run_migrations: bool,

    /// Registration mode: "open" (anyone), "closed" (admin only), "invite" (email confirmation)
    #[serde(default = "default_registration")]
    pub registration: String,

    /// Base URL for links in emails and OAuth callbacks (e.g. "https://pod.example.com")
    #[serde(default)]
    pub base_url: String,

    // --- SMTP ---
    #[serde(default)]
    pub smtp_host: String,
    #[serde(default = "default_smtp_port")]
    pub smtp_port: u16,
    #[serde(default)]
    pub smtp_user: String,
    #[serde(default)]
    pub smtp_password: String,
    #[serde(default)]
    pub smtp_from: String,
    /// "none", "starttls", "tls"
    #[serde(default = "default_smtp_security")]
    pub smtp_security: String,

    // --- OAuth2/OIDC ---
    #[serde(default)]
    pub oauth_issuer_url: String,
    #[serde(default)]
    pub oauth_client_id: String,
    #[serde(default)]
    pub oauth_client_secret: String,
    /// Display name for the SSO button (e.g. "Authentik", "Google")
    #[serde(default = "default_oauth_provider_name")]
    pub oauth_provider_name: String,
    /// OIDC group name that grants admin role (e.g. "admins")
    #[serde(default)]
    pub oauth_admin_group: String,

    // --- Sessions ---
    /// Session duration in days (default: 90)
    #[serde(default = "default_session_duration_days")]
    pub session_duration_days: u32,

    // --- Podcast Index API ---
    #[serde(default)]
    pub podcastindex_key: String,
    #[serde(default)]
    pub podcastindex_secret: String,

    // --- Metrics server ---
    /// Enable the dedicated metrics server (default: false)
    #[serde(default)]
    pub metrics_enabled: bool,

    /// Metrics server bind address (default: 0.0.0.0)
    #[serde(default = "default_host")]
    pub metrics_host: String,

    /// Metrics server bind port (default: 9091)
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,
}

fn default_database_url() -> String {
    "sqlite://rpodder.db".into()
}
fn default_host() -> String {
    "0.0.0.0".into()
}
fn default_port() -> u16 {
    3005
}
fn default_migrations_dir() -> String {
    "migrations".into()
}
fn default_registration() -> String {
    "open".into()
}
fn default_true() -> bool {
    true
}
fn default_smtp_port() -> u16 {
    25
}
fn default_smtp_security() -> String {
    "none".into()
}
fn default_oauth_provider_name() -> String {
    "SSO".into()
}
fn default_session_duration_days() -> u32 {
    90
}
fn default_metrics_port() -> u16 {
    9091
}

impl AppConfig {
    /// Load config from environment variables (RPODDER_*) and optional config file.
    pub fn load(config_file: Option<&str>) -> anyhow::Result<Self> {
        let mut builder = config::Config::builder();

        if let Some(path) = config_file {
            builder = builder.add_source(config::File::with_name(path).required(false));
        }

        builder = builder.add_source(
            config::Environment::with_prefix("RPODDER")
                .prefix_separator("_")
                .try_parsing(true),
        );

        let config = builder.build()?;
        Ok(config.try_deserialize()?)
    }

    pub fn smtp_configured(&self) -> bool {
        !self.smtp_host.is_empty()
    }

    pub fn oauth_configured(&self) -> bool {
        !self.oauth_issuer_url.is_empty() && !self.oauth_client_id.is_empty()
    }

    pub fn registration_invite(&self) -> bool {
        self.registration == "invite"
    }

    pub fn podcastindex_configured(&self) -> bool {
        !self.podcastindex_key.is_empty() && !self.podcastindex_secret.is_empty()
    }
}
