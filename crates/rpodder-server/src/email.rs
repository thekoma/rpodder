//! SMTP email sender for account activation.

use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use tracing::{error, info};

use crate::config::AppConfig;

/// Send an activation email with a confirmation link.
pub fn send_activation_email(
    config: &AppConfig,
    to_email: &str,
    username: &str,
    token: &str,
) -> Result<(), String> {
    if !config.smtp_configured() {
        return Err("SMTP not configured".into());
    }

    let base_url = if config.base_url.is_empty() {
        format!("http://{}:{}", config.host, config.port)
    } else {
        config.base_url.clone()
    };

    let activation_url = format!("{base_url}/api/2/activate?token={token}");

    let body = format!(
        "Hello {username},\n\n\
         Welcome to rpodder! Please activate your account by clicking the link below:\n\n\
         {activation_url}\n\n\
         If you did not create this account, you can safely ignore this email.\n\n\
         — rpodder"
    );

    let from = if config.smtp_from.is_empty() {
        format!("rpodder@{}", config.smtp_host)
    } else {
        config.smtp_from.clone()
    };

    let email = Message::builder()
        .from(from.parse().map_err(|e| format!("invalid from: {e}"))?)
        .to(to_email.parse().map_err(|e| format!("invalid to: {e}"))?)
        .subject("Activate your rpodder account")
        .header(ContentType::TEXT_PLAIN)
        .body(body)
        .map_err(|e| format!("failed to build email: {e}"))?;

    let transport = build_transport(config)?;

    match transport.send(&email) {
        Ok(_) => {
            info!(to = to_email, username, "activation email sent");
            Ok(())
        }
        Err(e) => {
            error!(to = to_email, error = %e, "failed to send activation email");
            Err(format!("SMTP error: {e}"))
        }
    }
}

/// Send a password reset email with a reset link.
pub fn send_password_reset_email(
    config: &AppConfig,
    to_email: &str,
    username: &str,
    token: &str,
) -> Result<(), String> {
    if !config.smtp_configured() {
        return Err("SMTP not configured".into());
    }

    let base_url = if config.base_url.is_empty() {
        format!("http://{}:{}", config.host, config.port)
    } else {
        config.base_url.clone()
    };

    let reset_url = format!("{base_url}/reset-password?token={token}");

    let body = format!(
        "Hello {username},\n\n\
         A password reset was requested for your rpodder account.\n\n\
         Click the link below to set a new password:\n\n\
         {reset_url}\n\n\
         This link expires in 24 hours.\n\n\
         If you did not request this, you can safely ignore this email.\n\n\
         — rpodder"
    );

    let from = if config.smtp_from.is_empty() {
        format!("rpodder@{}", config.smtp_host)
    } else {
        config.smtp_from.clone()
    };

    let email = Message::builder()
        .from(from.parse().map_err(|e| format!("invalid from: {e}"))?)
        .to(to_email.parse().map_err(|e| format!("invalid to: {e}"))?)
        .subject("Reset your rpodder password")
        .header(ContentType::TEXT_PLAIN)
        .body(body)
        .map_err(|e| format!("failed to build email: {e}"))?;

    let transport = build_transport(config)?;

    match transport.send(&email) {
        Ok(_) => {
            info!(to = to_email, username, "password reset email sent");
            Ok(())
        }
        Err(e) => {
            error!(to = to_email, error = %e, "failed to send password reset email");
            Err(format!("SMTP error: {e}"))
        }
    }
}

fn build_transport(config: &AppConfig) -> Result<SmtpTransport, String> {
    let builder = match config.smtp_security.as_str() {
        "tls" => {
            SmtpTransport::relay(&config.smtp_host).map_err(|e| format!("SMTP relay error: {e}"))?
        }
        "starttls" => SmtpTransport::starttls_relay(&config.smtp_host)
            .map_err(|e| format!("SMTP starttls error: {e}"))?,
        _ => SmtpTransport::builder_dangerous(&config.smtp_host),
    };

    let builder = builder.port(config.smtp_port);

    let builder = if !config.smtp_user.is_empty() {
        builder.credentials(Credentials::new(
            config.smtp_user.clone(),
            config.smtp_password.clone(),
        ))
    } else {
        builder
    };

    Ok(builder.build())
}
