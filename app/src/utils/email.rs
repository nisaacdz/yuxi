use crate::config::Config;
use lettre::{
    AsyncTransport, Message,
    message::{Mailbox, MultiPart, SinglePart, header::ContentType},
};

pub async fn send_email(
    config: &Config,
    to: &str,
    subject: &str,
    html_body: &str,
    text_body: &str,
) -> Result<(), anyhow::Error> {
    let email = Message::builder()
        .from(Mailbox::new(
            Some("Yuxi Team".to_string()),
            config.emailer.parse()?,
        ))
        .to(Mailbox::new(None, to.parse()?))
        .subject(subject)
        .multipart(
            MultiPart::alternative()
                .singlepart(
                    SinglePart::builder()
                        .header(ContentType::TEXT_PLAIN)
                        .body(text_body.to_string()),
                )
                .singlepart(
                    SinglePart::builder()
                        .header(ContentType::TEXT_HTML)
                        .body(html_body.to_string()),
                ),
        )?;

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
    let subject = "🔐 Password Reset Request - Yuxi";

    let html_body = format!(
        r#"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>Password Reset</title>
            <style>
                body {{
                    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
                    line-height: 1.6;
                    color: #333;
                    max-width: 600px;
                    margin: 0 auto;
                    padding: 20px;
                    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
                }}
                .email-container {{
                    background: white;
                    border-radius: 12px;
                    padding: 40px;
                    box-shadow: 0 10px 30px rgba(0,0,0,0.1);
                }}
                .header {{
                    text-align: center;
                    margin-bottom: 30px;
                }}
                .logo {{
                    font-size: 32px;
                    font-weight: bold;
                    color: #667eea;
                    margin-bottom: 10px;
                }}
                .title {{
                    font-size: 24px;
                    color: #2d3748;
                    margin-bottom: 10px;
                }}
                .subtitle {{
                    color: #718096;
                    font-size: 16px;
                }}
                .otp-section {{
                    background: #f7fafc;
                    border: 2px dashed #e2e8f0;
                    border-radius: 8px;
                    padding: 20px;
                    text-align: center;
                    margin: 30px 0;
                }}
                .otp-code {{
                    font-size: 36px;
                    font-weight: bold;
                    color: #667eea;
                    letter-spacing: 4px;
                    font-family: 'Courier New', monospace;
                    margin: 10px 0;
                }}
                .otp-label {{
                    color: #718096;
                    font-size: 14px;
                    text-transform: uppercase;
                    letter-spacing: 1px;
                    margin-bottom: 10px;
                }}
                .warning {{
                    background: #fed7d7;
                    color: #9b2c2c;
                    padding: 15px;
                    border-radius: 6px;
                    margin: 20px 0;
                    font-size: 14px;
                }}
                .footer {{
                    text-align: center;
                    margin-top: 30px;
                    padding-top: 20px;
                    border-top: 1px solid #e2e8f0;
                    color: #718096;
                    font-size: 14px;
                }}
                .button {{
                    display: inline-block;
                    background: #667eea;
                    color: white;
                    padding: 12px 24px;
                    text-decoration: none;
                    border-radius: 6px;
                    font-weight: 500;
                    margin: 20px 0;
                }}
                .expiry {{
                    color: #e53e3e;
                    font-weight: 500;
                    font-size: 14px;
                }}
            </style>
        </head>
        <body>
            <div class="email-container">
                <div class="header">
                    <div class="logo">🚀 Yuxi</div>
                    <h1 class="title">Password Reset Request</h1>
                    <p class="subtitle">We received a request to reset your password</p>
                </div>
                
                <div class="otp-section">
                    <div class="otp-label">Your Reset Code</div>
                    <div class="otp-code">{}</div>
                    <p class="expiry">⏰ Expires in 15 minutes</p>
                </div>
                
                <p>Enter this code in the password reset form to create a new password for your account.</p>
                
                <div class="warning">
                    <strong>⚠️ Security Notice:</strong> If you didn't request this password reset, please ignore this email. Your account remains secure.
                </div>
                
                <div class="footer">
                    <p>Need help? Contact our support team</p>
                    <p>© 2025 Yuxi. All rights reserved.</p>
                </div>
            </div>
        </body>
        </html>
        "#,
        otp
    );

    let text_body = format!(
        r#"
🚀 YUXI - Password Reset Request

We received a request to reset your password.

Your reset code: {}

⏰ This code expires in 15 minutes.

Enter this code in the password reset form to create a new password for your account.

⚠️ Security Notice: If you didn't request this password reset, please ignore this email. Your account remains secure.

Need help? Contact our support team.

© 2025 Yuxi. All rights reserved.
        "#,
        otp
    );

    send_email(config, to, subject, &html_body, &text_body).await
}

pub async fn send_welcome_email(
    config: &Config,
    to: &str,
    username: &str,
) -> Result<(), anyhow::Error> {
    let subject = "🎉 Welcome to Yuxi!";

    let html_body = format!(
        r###"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>Welcome to Yuxi</title>
            <style>
                body {{
                    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
                    line-height: 1.6;
                    color: #333;
                    max-width: 600px;
                    margin: 0 auto;
                    padding: 20px;
                    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
                }}
                .email-container {{
                    background: white;
                    border-radius: 12px;
                    padding: 40px;
                    box-shadow: 0 10px 30px rgba(0,0,0,0.1);
                }}
                .header {{
                    text-align: center;
                    margin-bottom: 30px;
                }}
                .logo {{
                    font-size: 32px;
                    font-weight: bold;
                    color: #667eea;
                    margin-bottom: 10px;
                }}
                .welcome-message {{
                    font-size: 24px;
                    color: #2d3748;
                    margin-bottom: 20px;
                    text-align: center;
                }}
                .cta-button {{
                    display: inline-block;
                    background: #667eea;
                    color: white;
                    padding: 15px 30px;
                    text-decoration: none;
                    border-radius: 8px;
                    font-weight: 500;
                    margin: 20px 0;
                    text-align: center;
                }}
                .footer {{
                    text-align: center;
                    margin-top: 30px;
                    padding-top: 20px;
                    border-top: 1px solid #e2e8f0;
                    color: #718096;
                    font-size: 14px;
                }}
            </style>
        </head>
        <body>
            <div class="email-container">
                <div class="header">
                    <div class="logo">🚀 Yuxi</div>
                    <h1 class="welcome-message">Welcome, {}! 🎉</h1>
                </div>
                
                <p>Thank you for joining Yuxi! We're excited to have you on board.</p>
                
                <p>Your account has been successfully created and you're ready to start your journey with us.</p>
                
                <div style="text-align: center;">
                    <a href="#" class="cta-button">Get Started</a>
                </div>
                
                <div class="footer">
                    <p>Need help getting started? Check out our documentation or contact support.</p>
                    <p>© 2025 Yuxi. All rights reserved.</p>
                </div>
            </div>
        </body>
        </html>
        "###,
        username
    );

    let text_body = format!(
        r#"
🚀 YUXI - Welcome!

Welcome, {}! 🎉

Thank you for joining Yuxi! We're excited to have you on board.

Your account has been successfully created and you're ready to start your journey with us.

Need help getting started? Check out our documentation or contact support.

© 2025 Yuxi. All rights reserved.
        "#,
        username
    );

    send_email(config, to, subject, &html_body, &text_body).await
}
