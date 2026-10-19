//! Google Desktop-OAuth: PKCE S256, einmaliger Loopback-Callback und Token-Lebenszyklus.
//!
//! Ohne Identitätsscope liefert Google keine stabile Konto-ID und keinen Kontonamen.
//! I02/I03 müssen in ApiSource::test_connection account_id und label aus der Property-/Site-Liste
//! bzw. Konto-Antwort setzen. Das generische Label darf nicht zur Zusammenführung dienen:
//! mehrere verschiedene Google-Identitäten können dieselben Scopes besitzen.
use crate::{credentials::Credential, ApiError};
use oauth2::{CsrfToken, PkceCodeChallenge, PkceCodeVerifier};
use os_model::{ApiKind, Clock};
use serde::Deserialize;
use std::{sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use url::Url;

#[derive(Clone)]
pub struct Endpoints {
    pub authorize: String,
    pub token: String,
    pub revoke: String,
}
impl Default for Endpoints {
    fn default() -> Self {
        Self {
            authorize: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token: "https://oauth2.googleapis.com/token".into(),
            revoke: "https://oauth2.googleapis.com/revoke".into(),
        }
    }
}
pub fn scope(kind: ApiKind) -> Result<&'static str, ApiError> {
    match kind {
        ApiKind::Ga4 => Ok("https://www.googleapis.com/auth/analytics.readonly"),
        ApiKind::Gsc => Ok("https://www.googleapis.com/auth/webmasters.readonly"),
        _ => Err(ApiError::NotImplemented("OAuth-Quelle")),
    }
}
pub struct Authorization {
    pub url: Url,
    redirect: String,
    listener: TcpListener,
    state: CsrfToken,
    verifier: PkceCodeVerifier,
}
impl Authorization {
    pub async fn begin(
        client_id: &str,
        kind: ApiKind,
        endpoints: &Endpoints,
    ) -> Result<Self, ApiError> {
        if client_id.trim().is_empty() {
            return Err(ApiError::Unauthorized(
                "Eigene Google-Client-ID erforderlich".into(),
            ));
        }
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(|_| transport())?;
        let redirect = format!(
            "http://127.0.0.1:{}/callback",
            listener.local_addr().map_err(|_| transport())?.port()
        );
        let state = CsrfToken::new_random();
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let mut url = Url::parse(&endpoints.authorize).map_err(|_| transport())?;
        url.query_pairs_mut().extend_pairs([
            ("response_type", "code"),
            ("client_id", client_id),
            ("redirect_uri", redirect.as_str()),
            ("scope", scope(kind)?),
            ("state", state.secret()),
            ("code_challenge", challenge.as_str()),
            ("code_challenge_method", "S256"),
            ("access_type", "offline"),
            ("prompt", "consent"),
        ]);
        Ok(Self {
            url,
            redirect,
            listener,
            state,
            verifier,
        })
    }
    pub fn open_browser(&self) -> Result<(), ApiError> {
        webbrowser::open(self.url.as_str())
            .map(|_| ())
            .map_err(|_| ApiError::Transport("Systembrowser konnte nicht geöffnet werden".into()))
    }
    /// Testzugang und Desktop benutzen denselben einmaligen Rückruf.
    pub async fn finish(
        self,
        client: &reqwest::Client,
        client_id: &str,
        client_secret: Option<&str>,
        endpoints: &Endpoints,
        clock: Arc<dyn Clock>,
    ) -> Result<Credential, ApiError> {
        let code = tokio::time::timeout(Duration::from_secs(300), self.receive())
            .await
            .map_err(|_| {
                ApiError::Unauthorized("OAuth-Zeitüberschreitung nach 5 Minuten".into())
            })??;
        let Authorization {
            redirect,
            listener,
            verifier,
            ..
        } = self;
        drop(listener);
        let mut form = vec![
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("client_id", client_id),
            ("redirect_uri", redirect.as_str()),
            ("code_verifier", verifier.secret()),
        ];
        if let Some(secret) = client_secret {
            form.push(("client_secret", secret));
        }
        let token = token_request(client, &endpoints.token, &form).await?;
        let label = token.account.unwrap_or_else(|| "Google-Konto".into());
        // Ohne Identitätsscope gibt Google keine E-Mail zurück. Die lokale Kennung ist zufällig.
        let id = token
            .account_id
            .unwrap_or_else(|| CsrfToken::new_random().secret().clone());
        Ok(Credential {
            account_id: id,
            label,
            api_key: None,
            access_token: Some(token.access_token),
            refresh_token: token.refresh_token,
            expires_at: Some(clock.now().plus(Duration::from_secs(token.expires_in))),
            client_secret: client_secret.map(str::to_string),
            scopes: token
                .scope
                .unwrap_or_default()
                .split_whitespace()
                .map(str::to_string)
                .collect(),
        })
    }
    async fn receive(&self) -> Result<String, ApiError> {
        loop {
            let (mut stream, _) = self.listener.accept().await.map_err(|_| transport())?;
            // Browser-Preconnects dürfen den Callback nicht bis zum Gesamt-Timeout blockieren.
            let request =
                tokio::time::timeout(Duration::from_secs(2), read_request(&mut stream)).await;
            let request = match request {
                Ok(Ok(request)) => request,
                _ => {
                    reply(&mut stream, false, false).await;
                    continue;
                }
            };
            let mut parts = request
                .lines()
                .next()
                .unwrap_or_default()
                .split_whitespace();
            let method = parts.next();
            let path = parts.next().unwrap_or_default();
            let callback = Url::parse(&format!("http://127.0.0.1{path}"));
            let callback = match callback {
                Ok(url) if method == Some("GET") && url.path() == "/callback" => url,
                _ => {
                    reply(&mut stream, false, true).await;
                    continue;
                }
            };
            let parameters: Vec<_> = callback.query_pairs().collect();
            let states: Vec<_> = parameters.iter().filter(|(k, _)| k == "state").collect();
            let valid = states.len() == 1
                && states[0].1 == *self.state.secret()
                && parameters.iter().filter(|(key, _)| key == "code").count() == 1
                && !parameters.iter().any(|(key, _)| key == "error");
            let result = if valid {
                parameters
                    .iter()
                    .find(|(k, _)| k == "code")
                    .map(|(_, v)| v.to_string())
                    .filter(|code| !code.is_empty())
                    .ok_or_else(|| ApiError::Unauthorized("OAuth-Zustimmung verweigert".into()))
            } else {
                Err(ApiError::Unauthorized(
                    "OAuth-Rückruf: ungültiger state".into(),
                ))
            };
            reply(&mut stream, result.is_ok(), false).await;
            return result;
        }
    }
}
async fn read_request(stream: &mut tokio::net::TcpStream) -> Result<String, ApiError> {
    let mut bytes = Vec::new();
    loop {
        let mut part = [0; 1024];
        let n = stream.read(&mut part).await.map_err(|_| transport())?;
        if n == 0 {
            return Err(transport());
        }
        bytes.extend_from_slice(&part[..n]);
        if bytes.len() > 16384 {
            return Err(transport());
        }
        if bytes.windows(4).any(|w| w == b"\r\n\r\n") {
            return String::from_utf8(bytes).map_err(|_| transport());
        }
    }
}
async fn reply(stream: &mut tokio::net::TcpStream, success: bool, missing: bool) {
    let response = if success {
        "HTTP/1.1 200 OK\r\nContent-Length: 23\r\nConnection: close\r\n\r\nAnmeldung abgeschlossen"
    } else if missing {
        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    } else {
        "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    };
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}
#[derive(Deserialize)]
struct Token {
    access_token: String,
    refresh_token: Option<String>,
    #[serde(default = "expiry")]
    expires_in: u64,
    scope: Option<String>,
    account: Option<String>,
    account_id: Option<String>,
}
fn expiry() -> u64 {
    3600
}
fn transport() -> ApiError {
    ApiError::Transport("OAuth-Verbindung fehlgeschlagen".into())
}
async fn token_request(
    client: &reqwest::Client,
    endpoint: &str,
    form: &[(&str, &str)],
) -> Result<Token, ApiError> {
    let body = encode_form(form);
    let response = client
        .post(endpoint)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|_| transport())?;
    if !response.status().is_success() {
        return Err(ApiError::Unauthorized(
            "Anmeldung abgelaufen – bitte neu verbinden".into(),
        ));
    }
    serde_json::from_slice(&response.bytes().await.map_err(|_| transport())?)
        .map_err(|_| transport())
}
pub async fn refresh(
    credential: &mut Credential,
    client: &reqwest::Client,
    client_id: &str,
    endpoints: &Endpoints,
    clock: &dyn Clock,
) -> Result<(), ApiError> {
    if credential
        .expires_at
        .is_some_and(|at| at > clock.now().plus(Duration::from_secs(60)))
    {
        return Ok(());
    }
    let refresh = credential.refresh_token.as_deref().ok_or_else(|| {
        ApiError::Unauthorized("Anmeldung abgelaufen – bitte neu verbinden".into())
    })?;
    let mut form = vec![
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh),
        ("client_id", client_id),
    ];
    if let Some(secret) = credential.client_secret.as_deref() {
        form.push(("client_secret", secret));
    }
    let token = token_request(client, &endpoints.token, &form).await?;
    credential.access_token = Some(token.access_token);
    if let Some(refresh) = token.refresh_token {
        credential.refresh_token = Some(refresh);
    }
    credential.expires_at = Some(clock.now().plus(Duration::from_secs(token.expires_in)));
    Ok(())
}
pub async fn revoke(
    credential: &Credential,
    client: &reqwest::Client,
    endpoints: &Endpoints,
) -> Result<(), ApiError> {
    let token = credential
        .refresh_token
        .as_ref()
        .or(credential.access_token.as_ref());
    if let Some(token) = token {
        let body = encode_form(&[("token", token.as_str())]);
        let response = client
            .post(&endpoints.revoke)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .map_err(|_| transport())?;
        if !response.status().is_success() {
            return Err(transport());
        }
    }
    Ok(())
}

fn encode_form(form: &[(&str, &str)]) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(form.iter().copied())
        .finish()
}
