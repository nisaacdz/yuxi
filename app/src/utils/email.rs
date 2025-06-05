use crate::config::Config;

pub async fn send_email(config: Config, to: &str, subject: &str, body: &str) -> Result<(), anyhow::Error> {
    use lettre::{Message, Transport};
    use lettre::message::Mailbox;
    
    let email = Message::builder()
        .from(Mailbox::new(None, config.emailer.parse()?))
        .to(Mailbox::new(None, to.parse()?))
        .subject(subject)
        .body(body.to_string())?;

    config.transponder.send(email).await.map_err(|e| {
        anyhow::anyhow!("Failed to send email: {}", e)
    })?;
    
    Ok(())
}