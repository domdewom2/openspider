//! Ahrefs Connect Desktop-OAuth: PKCE, ein einmaliger Loopback-Rückruf, keine Refresh-Tokens.
use crate::{credentials::Credential, ApiError};
use oauth2::{CsrfToken, PkceCodeChallenge, PkceCodeVerifier};
use os_model::Clock;
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
}
impl Default for Endpoints {
    fn default() -> Self {
        Self {
            authorize: "https://app.ahrefs.com/web/oauth/authorize".into(),
            token: "https://ahrefs.com/oauth/token".into(),
        }
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
    pub async fn begin(client_id: &str, endpoints: &Endpoints) -> Result<Self, ApiError> {
        if client_id.trim().is_empty() {
            return Err(ApiError::Unauthorized(
                "Eigene Ahrefs-Client-ID erforderlich".into(),
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
            ("scope", "apiv3-integration-apps"),
            ("state", state.secret()),
            ("code_challenge", challenge.as_str()),
            ("code_challenge_method", "s256"),
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
        let label = token.account.unwrap_or_else(|| "Ahrefs-Konto".into());
        // Ohne Identitätsscope gibt Ahrefs keine E-Mail zurück. Die lokale Kennung ist zufällig.
        let id = token
            .account_id
            .unwrap_or_else(|| CsrfToken::new_random().secret().clone());
        Ok(Credential {
            account_id: id,
            label,
            api_key: None,
            access_token: Some(token.access_token),
            refresh_token: None,
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
    #[serde(default = "expiry")]
    expires_in: u64,
    scope: Option<String>,
    account: Option<String>,
    account_id: Option<String>,
}
fn expiry() -> u64 {
    365 * 24 * 3600
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

fn encode_form(form: &[(&str, &str)]) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(form.iter().copied())
        .finish()
}
