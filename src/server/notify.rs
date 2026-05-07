//! OTP notification — email via SMTP and SMS via Twilio.

use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor, message::Mailbox,
    transport::smtp::authentication::Credentials,
};
use reqwest::Client;
use tracing::{debug, error, info};

use crate::server::config::{AppConfig, EmailConfig, SmsConfig};

/// Opaque error returned by notification sends and setup.
#[derive(Debug, thiserror::Error)]
pub enum NotifyError {
    #[error("email delivery failed: {0}")]
    Email(String),
    #[error("SMS delivery failed: {0}")]
    Sms(String),
    #[error("no delivery channel configured for contact type '{0}'")]
    Unconfigured(&'static str),
}

/// Contact classification used when routing a send.
#[derive(Debug, Clone, Copy)]
pub enum ContactType {
    Email,
    Phone,
}

struct SmtpNotifier {
    mailer: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

impl SmtpNotifier {
    fn new(cfg: &EmailConfig) -> Result<Self, NotifyError> {
        if cfg.smtp_username.is_empty() && !cfg.smtp_password.is_empty() {
            return Err(NotifyError::Email(
                "smtp_password is set but smtp_username is empty".into(),
            ));
        }

        let from = cfg
            .from_address
            .parse::<Mailbox>()
            .map_err(|e| NotifyError::Email(format!("invalid from_address: {e}")))?;

        let mut builder = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.smtp_host)
            .map_err(|e| NotifyError::Email(e.to_string()))?
            .port(cfg.smtp_port);

        if !cfg.smtp_username.is_empty() {
            builder = builder.credentials(Credentials::new(
                cfg.smtp_username.clone(),
                cfg.smtp_password.clone(),
            ));
        }

        Ok(Self {
            mailer: builder.build(),
            from,
        })
    }

    async fn send(&self, to: &str, purpose: &str, code: &str) -> Result<(), NotifyError> {
        let (subject, action) = match purpose {
            "password_reset" => (
                "Your jogga password reset code",
                "reset your jogga password",
            ),
            _ => ("Your jogga verification code", "verify your jogga account"),
        };

        let recipient = to
            .parse::<Mailbox>()
            .map_err(|e| NotifyError::Email(format!("invalid recipient address: {e}")))?;
        let body = format!("Your code to {action} is {code}.\n\nThis code expires in 15 minutes.");
        let message = Message::builder()
            .from(self.from.clone())
            .to(recipient)
            .subject(subject)
            .body(body)
            .map_err(|e| NotifyError::Email(e.to_string()))?;

        self.mailer
            .send(message)
            .await
            .map_err(|e| NotifyError::Email(e.to_string()))?;
        Ok(())
    }
}

struct SmsNotifier {
    http: Client,
    account_sid: String,
    auth_token: String,
    from: String,
}

impl SmsNotifier {
    fn new(cfg: &SmsConfig, http: Client) -> Self {
        Self {
            http,
            account_sid: cfg.account_sid.clone(),
            auth_token: cfg.auth_token.clone(),
            from: cfg.from_number.clone(),
        }
    }

    async fn send(&self, to: &str, purpose: &str, code: &str) -> Result<(), NotifyError> {
        let verb = match purpose {
            "password_reset" => "reset your password on",
            _ => "verify your account on",
        };
        let body = format!("Your code to {verb} jogga: {code}  (expires in 15 min)");

        let url = format!(
            "https://api.twilio.com/2010-04-01/Accounts/{}/Messages.json",
            self.account_sid
        );
        let resp = self
            .http
            .post(&url)
            .basic_auth(&self.account_sid, Some(&self.auth_token))
            .form(&[("To", to), ("From", &self.from), ("Body", &body)])
            .send()
            .await
            .map_err(|e| NotifyError::Sms(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(NotifyError::Sms(format!("Twilio {status}: {text}")));
        }
        Ok(())
    }
}

pub struct AppNotifier {
    email: Option<SmtpNotifier>,
    sms: Option<SmsNotifier>,
}

impl AppNotifier {
    pub fn new(config: &AppConfig, http: Client) -> Result<Self, NotifyError> {
        let email = config.email.as_deref().map(SmtpNotifier::new).transpose()?;
        let sms = config.sms.as_deref().map(|c| SmsNotifier::new(c, http));
        Ok(Self { email, sms })
    }

    /// Send an OTP code to the given contact.
    pub async fn send(
        &self,
        contact: &str,
        contact_type: ContactType,
        purpose: &str,
        code: &str,
    ) -> Result<(), NotifyError> {
        match contact_type {
            ContactType::Email => {
                let notifier = self
                    .email
                    .as_ref()
                    .ok_or(NotifyError::Unconfigured("email"))?;
                debug!(purpose, "sending OTP email via SMTP");
                match notifier.send(contact, purpose, code).await {
                    Ok(()) => {
                        info!(purpose, "OTP email sent successfully");
                        Ok(())
                    }
                    Err(e) => {
                        error!(error = %e, purpose, "OTP email delivery failed");
                        Err(e)
                    }
                }
            }
            ContactType::Phone => {
                let notifier = self
                    .sms
                    .as_ref()
                    .ok_or(NotifyError::Unconfigured("phone"))?;
                debug!(purpose, "sending OTP SMS");
                match notifier.send(contact, purpose, code).await {
                    Ok(()) => {
                        info!(purpose, "OTP SMS sent successfully");
                        Ok(())
                    }
                    Err(e) => {
                        error!(error = %e, purpose, "OTP SMS delivery failed");
                        Err(e)
                    }
                }
            }
        }
    }
}
