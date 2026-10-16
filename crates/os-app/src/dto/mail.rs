//! E-Mail-Konten (`mail.*`, OUT-280).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::error::CmdError;

/// Verschlüsselung der Verbindung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum MailTlsDto {
    /// STARTTLS.
    Starttls,
    /// Implizites TLS.
    Tls,
    /// Unverschlüsselt (nur nach ausdrücklicher Bestätigung).
    None,
}

/// Ein Konto ohne Kennwort.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MailAccountInfo {
    /// Kennung.
    pub id: String,
    /// Bezeichnung.
    pub label: String,
    /// SMTP-Host.
    pub host: String,
    /// Port.
    pub port: u16,
    /// Verschlüsselung.
    pub tls: MailTlsDto,
    /// Absenderadresse.
    pub from: String,
    /// Ein Kennwort ist hinterlegt.
    pub has_secret: bool,
}

/// Eingabe zum Anlegen oder Ändern; das Kennwort steht nur hier und kommt in den Schlüsselbund.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct MailAccountInput {
    /// Kennung; `null` bei neuen Konten.
    pub id: Option<String>,
    /// Bezeichnung.
    pub label: String,
    /// SMTP-Host.
    pub host: String,
    /// Port.
    pub port: u16,
    /// Verschlüsselung.
    pub tls: MailTlsDto,
    /// Benutzername.
    pub username: String,
    /// Absenderadresse.
    pub from: String,
    /// Kennwort; `null` = unverändert lassen.
    pub password: Option<String>,
}

/// Argument von `mail.saveAccount`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MailSaveAccountArgs {
    /// Konto.
    pub account: MailAccountInput,
}

/// Argument von `mail.testAccount`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MailTestArgs {
    /// Konto.
    pub id: String,
    /// Empfänger der Testmail.
    pub to: String,
}

/// Ergebnis von `mail.testAccount`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct MailTestResult {
    /// Zugestellt.
    pub ok: bool,
    /// Fehler mit SMTP-Antwortcode in `args`.
    pub error: Option<CmdError>,
}
