//! Versand fertiger Abschlussmail-Inhalte; Kontenverwaltung bleibt bei T02.
use lettre::message::{
    header::{ContentTransferEncoding, ContentType},
    Mailbox, MultiPart, SinglePart,
};
use lettre::{Message, SmtpTransport, Transport};

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
