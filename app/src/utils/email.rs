use crate::config::Config;
use lettre::{AsyncTransport, Message, message::Mailbox};

pub async fn send_email(
    config: &Config,
    to: &str,
    subject: &str,
    body: &str,
) -> Result<(), anyhow::Error> {
    let email = Message::builder()
        .from(Mailbox::new(None, config.emailer.parse()?))
        .to(Mailbox::new(None, to.parse()?))
        .subject(subject)
        .body(body.to_string())?;

    config
        .transponder
        .send(email)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to send email: {}", e))?;
    Ok(())
}

pub async fn send_forgot_password_email(
    config: &Config,
    to: &str,
    otp: &str,
) -> Result<(), anyhow::Error> {
    let subject = "Password Reset Request";
    let body = format!("Your OTP for password reset is: {}", otp);
    send_email(config, to, subject, &body).await
}
