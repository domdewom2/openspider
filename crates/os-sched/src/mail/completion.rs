//! Versand fertiger Abschlussmail-Inhalte; Kontenverwaltung bleibt bei T02.
use lettre::message::{
    header::{ContentTransferEncoding, ContentType},
    Mailbox, MultiPart, SinglePart,
};
use lettre::{Message, SmtpTransport, Transport};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// SMTP-Metadaten in `settings/mail/<account-id>.json`, ohne Kennwort.
/// Das Kennwort liegt ausschließlich unter `mail/<account-id>` im SecretStore.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SmtpAccount {
    pub host: String,
    pub port: u16,
    pub tls: SmtpTls,
    pub username: String,
    pub from: String,
    /// Explizite Bestätigung für unverschlüsseltes SMTP (OUT-280).
    #[serde(default)]
    pub allow_insecure: bool,
}

/// Transportverschlüsselung; dieselben Werte wie im Mail-Konten-DTO.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SmtpTls {
    Starttls,
    Tls,
    None,
}

/// Versandgrenze für Klartext und HTML derselben Abschlussmail.
pub trait CompletionSender: Send + Sync {
    /// Sendet eine Multipart-Alternative an die gewählten Empfänger.
    fn send(&self, to: &[String], subject: &str, text: &str, html: &str) -> Result<(), String>;
}

/// SMTP-Adapter; der Aufrufer konfiguriert TLS und Credentials im Transport.
pub struct SmtpCompletionSender {
    transport: SmtpTransport,
    from: Mailbox,
}

impl SmtpCompletionSender {
    /// Löst ein gespeichertes SMTP-Konto und dessen Kennwort auf (OUT-152).
    /// Konto-IDs werden vor dem Dateizugriff geprüft; Fehler enthalten keine Zugangsdaten.
    pub fn from_account(
        settings: &Path,
        id: &str,
        secrets: &dyn os_fetch::SecretStore,
    ) -> Result<Self, String> {
        if !crate::tasks::store::valid_id(id) {
            return Err("Completion mail: invalid account ID".into());
        }
        let json = std::fs::read(settings.join("mail").join(format!("{id}.json")))
            .map_err(|_| "Completion mail: SMTP account not readable")?;
        let account: SmtpAccount = serde_json::from_slice(&json)
            .map_err(|_| "Completion mail: invalid SMTP account configuration")?;
        if account.host.trim().is_empty() || account.port == 0 {
            return Err("Completion mail: SMTP host and port are required".into());
        }
        let builder = match account.tls {
            SmtpTls::Starttls => SmtpTransport::starttls_relay(&account.host),
            SmtpTls::Tls => SmtpTransport::relay(&account.host),
            SmtpTls::None if account.allow_insecure => {
                Ok(SmtpTransport::builder_dangerous(&account.host))
            }
            SmtpTls::None => {
                return Err(
                    "Completion mail: unencrypted SMTP requires explicit confirmation".into(),
                )
            }
        }
        .map_err(|_| "Completion mail: could not configure SMTP transport")?;
        let password = secrets
            .try_get(&format!("mail/{id}"))
            .map_err(|_| "Completion mail: SMTP credential store unavailable")?
            .ok_or("Completion mail: SMTP credential missing")?;
        let transport = builder
            .port(account.port)
            .timeout(Some(std::time::Duration::from_secs(10)))
            .credentials(lettre::transport::smtp::authentication::Credentials::new(
                account.username,
                password,
            ))
            .build();
        Self::new(transport, &account.from)
    }

    /// Übernimmt einen konfigurierten Transport (Credentials aus SecretStore).
    pub fn new(transport: SmtpTransport, from: &str) -> Result<Self, String> {
        Ok(Self {
            transport,
            from: from.parse::<Mailbox>().map_err(|e| e.to_string())?,
        })
    }

    /// Unverschlüsselter Loopback-Transport für eine lokale SMTP-Fixture.
    pub fn loopback(port: u16, from: &str) -> Result<Self, String> {
        Self::new(
            SmtpTransport::builder_dangerous("127.0.0.1")
                .port(port)
                .timeout(Some(std::time::Duration::from_secs(5)))
                .build(),
            from,
        )
    }
}

impl CompletionSender for SmtpCompletionSender {
    fn send(&self, to: &[String], subject: &str, text: &str, html: &str) -> Result<(), String> {
        let mut message = Message::builder().from(self.from.clone()).subject(subject);
        for recipient in to {
            message = message.to(recipient.parse::<Mailbox>().map_err(|e| e.to_string())?);
        }
        let message = message
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .header(ContentTransferEncoding::QuotedPrintable)
                            .body(text.to_owned()),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .header(ContentTransferEncoding::QuotedPrintable)
                            .body(html.to_owned()),
                    ),
            )
            .map_err(|e| e.to_string())?;
        self.transport.send(&message).map_err(|e| e.to_string())?;
        Ok(())
    }
}
