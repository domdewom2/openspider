//! Authentifizierungsprofile und formularbasierte Anmeldung (`auth.*`, OUT-223, CRAWL-120/121).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Art eines Profils.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum AuthProfileKind {
    /// Formular-Login (Cookies, Local/Session Storage).
    Form,
    /// Basic.
    Basic,
    /// Digest.
    Digest,
    /// NTLM.
    Ntlm,
}

/// Ein Profil ohne Geheimnisse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AuthProfileInfo {
    /// Name.
    pub name: String,
    /// Art.
    pub kind: AuthProfileKind,
    /// Host bzw. URL-Präfix, für den das Profil gilt.
    pub host: String,
    /// Angelegt (ISO 8601 UTC).
    pub created_at: String,
    /// Ein Geheimnis ist hinterlegt.
    pub has_secret: bool,
}

/// Eingabe zum Anlegen oder Ändern; das Geheimnis steht nur hier und wird verschlüsselt abgelegt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct AuthProfileInput {
    /// Name.
    pub name: String,
    /// Art.
    pub kind: AuthProfileKind,
    /// Host bzw. URL-Präfix.
    pub host: String,
    /// Benutzername.
    pub username: Option<String>,
    /// Kennwort bzw. Sitzungsdaten; nie in Antworten.
    pub secret: Option<String>,
}

/// Argument von `auth.saveProfile`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AuthSaveProfileArgs {
    /// Profil.
    pub profile: AuthProfileInput,
}

/// Fenstergröße des Login-Browsers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ViewportDto {
    /// Breite.
    pub width: u32,
    /// Höhe.
    pub height: u32,
}

/// Argument von `auth.openLogin` (CRAWL-120).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct AuthOpenLoginArgs {
    /// Login-URL.
    pub url: String,
    /// Bestehendes Profil, dessen Sitzung erneuert wird.
    pub profile: Option<String>,
    /// Fenstergröße.
    pub viewport: Option<ViewportDto>,
}

/// Ergebnis von `auth.openLogin`: Anzahlen; die Inhalte bleiben im Kern.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LoginCapture {
    /// URL nach dem Login.
    pub final_url: String,
    /// Anzahl Cookies.
    pub cookies: u32,
    /// Einträge im Local Storage.
    pub local_storage: u32,
    /// Einträge im Session Storage.
    pub session_storage: u32,
}

/// Argument von `auth.finishLogin`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct AuthFinishLoginArgs {
    /// Operation von `auth.openLogin`.
    pub op_id: String,
    /// Als Profil speichern.
    pub save_as: Option<String>,
}

/// Command-Vertrag AuthImportArgs (M2.md 3.14.9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AuthImportArgs {
    /// Vertragserweiterung M2-F05b: path.
    pub path: String,

    /// Vertragserweiterung M2-F05b: passphrase.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub passphrase: Option<String>,

    /// Vertragserweiterung M2-F05b: bind_to_machine.
    pub bind_to_machine: bool,
}

/// Command-Vertrag AuthExportArgs (M2.md 3.14.9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AuthExportArgs {
    /// Vertragserweiterung M2-F05b: name.
    pub name: String,

    /// Vertragserweiterung M2-F05b: path.
    pub path: String,

    /// Vertragserweiterung M2-F05b: passphrase.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub passphrase: Option<String>,

    /// Vertragserweiterung M2-F05b: bind_to_machine.
    pub bind_to_machine: bool,
}
