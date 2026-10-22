use super::{Account, Auth, Completion, MailError, Tls};
use lettre::{
    message::{MultiPart, SinglePart},
    transport::smtp::{
        authentication::{Credentials, Mechanism},
        client::{AsyncSmtpConnection, AsyncTokioStream},
        extension::ClientId,
    },
    Message,
};
use os_fetch::secrets::SecretStoreMut;
use os_model::{settings::NetworkSettings, ProxyKind, ProxyMode};
use std::{
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};

trait Io: AsyncRead + AsyncWrite + Send + Sync + Unpin {}
impl<T: AsyncRead + AsyncWrite + Send + Sync + Unpin> Io for T {}
struct Stream {
    io: Box<dyn Io>,
    peer: SocketAddr,
}
impl std::fmt::Debug for Stream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SMTP stream")
    }
}
impl AsyncRead for Stream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        b: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut *self.io).poll_read(cx, b)
    }
}
impl AsyncWrite for Stream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        b: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut *self.io).poll_write(cx, b)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut *self.io).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut *self.io).poll_shutdown(cx)
    }
}
impl AsyncTokioStream for Stream {
    fn peer_addr(&self) -> std::io::Result<SocketAddr> {
        Ok(self.peer)
    }
}
fn smtp(e: lettre::transport::smtp::Error) -> MailError {
    let code = e.status().map(|s| s.to_string());
    MailError {
        message: format!(
            "SMTP-Versand fehlgeschlagen{}{}",
            if e.is_permanent() {
                ": Anmeldung oder Nachricht abgelehnt"
            } else {
                ""
            },
            code.as_ref()
                .map(|s| format!(" (SMTP {s})"))
                .unwrap_or_default()
        ),
        smtp_code: code,
    }
}
fn b64(bytes: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in bytes.chunks(3) {
        let v = ((c[0] as u32) << 16)
            | ((c.get(1).copied().unwrap_or(0) as u32) << 8)
            | c.get(2).copied().unwrap_or(0) as u32;
        out.push(T[((v >> 18) & 63) as usize] as char);
        out.push(T[((v >> 12) & 63) as usize] as char);
        out.push(if c.len() > 1 {
            T[((v >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            T[(v & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}
async fn line(s: &mut Stream) -> Result<String, MailError> {
    let mut out = vec![];
    loop {
        let b = s.read_u8().await?;
        out.push(b);
        if out.len() > 8192 {
            return Err(MailError::new("Proxy-Antwort zu groß"));
        }
        if b == b'\n' {
            break;
        }
    }
    String::from_utf8(out).map_err(|_| MailError::new("Proxy-Antwort ungültig"))
}
fn bypass(host: &str, patterns: &[String]) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    patterns.iter().any(|p| {
        let p = p.trim().trim_start_matches('.').to_ascii_lowercase();
        p == "*"
            || host == p
            || (host.parse::<std::net::IpAddr>().is_err() && host.ends_with(&format!(".{p}")))
    })
}
async fn tunnel(
    a: &Account,
    n: &NetworkSettings,
    secrets: &dyn SecretStoreMut,
) -> Result<Stream, MailError> {
    let mut proxy = n.proxy.clone();
    if proxy.mode == ProxyMode::System {
        // The common resolver also covers platform proxy settings (K2-33).
        proxy = os_fetch::net::mail_system_proxy(&a.host, a.port).unwrap_or_else(|| {
            os_model::settings::ProxySettings {
                mode: ProxyMode::None,
                ..Default::default()
            }
        });
    }
    let use_proxy = proxy.mode == ProxyMode::Manual && !bypass(&a.host, &proxy.bypass);
    let (host, port) = if use_proxy {
        (proxy.host.as_str(), proxy.port)
    } else {
        (a.host.as_str(), a.port)
    };
    let tcp = tokio::net::TcpStream::connect((host, port)).await?;
    let peer = tcp.peer_addr()?;
    let mut s = Stream {
        io: Box::new(tcp),
        peer,
    };
    if use_proxy {
        let password = secrets
            .try_get(os_fetch::net::PROXY_PASSWORD)
            .map_err(|_| MailError::new("Proxy-Schlüsselbund nicht lesbar"))?;
        if proxy.kind == ProxyKind::Https {
            s = Stream {
                io: Box::new(
                    os_fetch::net::tls_tunnel(s, &proxy.host, n)
                        .await
                        .map_err(MailError::new)?,
                ),
                peer,
            };
        }
        match proxy.kind {
            ProxyKind::Http | ProxyKind::Https => {
                let target = if a.host.contains(':') {
                    format!("[{}]:{}", a.host, a.port)
                } else {
                    format!("{}:{}", a.host, a.port)
                };
                let auth = proxy
                    .username
                    .as_ref()
                    .map(|u| {
                        format!(
                            "Proxy-Authorization: Basic {}\r\n",
                            b64(format!("{u}:{}", password.as_deref().unwrap_or("")).as_bytes())
                        )
                    })
                    .unwrap_or_default();
                s.write_all(
                    format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n{auth}\r\n").as_bytes(),
                )
                .await?;
                let status = line(&mut s).await?;
                let code = status.split_whitespace().nth(1).unwrap_or("");
                if code != "200" {
                    return Err(MailError::new(if code == "407" {
                        "Proxy-Authentifizierung fehlgeschlagen (407)"
                    } else {
                        "Proxy-Tunnel abgelehnt"
                    }));
                }
                let mut size = 0;
                loop {
                    let l = line(&mut s).await?;
                    size += l.len();
                    if size > 32768 {
                        return Err(MailError::new("Proxy-Antwort zu groß"));
                    }
                    if l == "\r\n" {
                        break;
                    }
                }
            }
            ProxyKind::Socks5 => {
                s.write_all(if proxy.username.is_some() {
                    &[5, 1, 2]
                } else {
                    &[5, 1, 0]
                })
                .await?;
                let mut reply = [0; 2];
                s.read_exact(&mut reply).await?;
                let method = if proxy.username.is_some() { 2 } else { 0 };
                if reply != [5, method] {
                    return Err(MailError::new("SOCKS5-Authentifizierung abgelehnt"));
                }
                if let Some(user) = &proxy.username {
                    let pass = password.as_deref().unwrap_or("");
                    if user.len() > 255 || pass.len() > 255 {
                        return Err(MailError::new("SOCKS5-Zugangsdaten zu lang"));
                    }
                    let mut auth = vec![1, user.len() as u8];
                    auth.extend(user.as_bytes());
                    auth.push(pass.len() as u8);
                    auth.extend(pass.as_bytes());
                    s.write_all(&auth).await?;
                    s.read_exact(&mut reply).await?;
                    if reply != [1, 0] {
                        return Err(MailError::new("SOCKS5-Authentifizierung fehlgeschlagen"));
                    }
                }
                if a.host.len() > 255 {
                    return Err(MailError::new("SMTP-Hostname zu lang"));
                }
                let mut request = vec![5, 1, 0, 3, a.host.len() as u8];
                request.extend(a.host.as_bytes());
                request.extend(a.port.to_be_bytes());
                s.write_all(&request).await?;
                let mut head = [0; 4];
                s.read_exact(&mut head).await?;
                if head[0] != 5 || head[1] != 0 {
                    return Err(MailError::new("SOCKS5-Tunnel abgelehnt"));
                }
                let count = match head[3] {
                    1 => 4,
                    4 => 16,
                    3 => s.read_u8().await? as usize,
                    _ => return Err(MailError::new("SOCKS5-Antwort ungültig")),
                };
                let mut rest = vec![0; count + 2];
                s.read_exact(&mut rest).await?;
            }
        }
    }
    Ok(s)
}

pub async fn send(
    a: &Account,
    secrets: &dyn SecretStoreMut,
    n: &NetworkSettings,
    to: &[String],
    completion: &Completion,
) -> Result<(), MailError> {
    tokio::time::timeout(
        Duration::from_secs(30),
        send_inner(a, secrets, n, to, completion),
    )
    .await
    .map_err(|_| MailError::new("Mail-Versand: Zeitüberschreitung nach 30 Sekunden"))?
}
async fn send_inner(
    a: &Account,
    secrets: &dyn SecretStoreMut,
    n: &NetworkSettings,
    to: &[String],
    c: &Completion,
) -> Result<(), MailError> {
    a.validate()?;
    let (mut text, mut html) = c.render();
    let mut subject = c.subject();
    // Never include credentials even if an upstream error or user-supplied label repeats them.
    for key in [
        format!("mail/{}", a.id),
        os_fetch::net::PROXY_PASSWORD.into(),
    ] {
        if let Some(raw) = secrets
            .try_get(&key)
            .map_err(|_| MailError::new("Schlüsselbund nicht lesbar"))?
        {
            let mut values = vec![raw.clone()];
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
                for field in ["access_token", "refresh_token", "client_secret"] {
                    if let Some(token) = value[field].as_str() {
                        values.push(token.into());
                    }
                }
            }
            for value in values.into_iter().filter(|s| !s.is_empty()) {
                text = text.replace(&value, "<redacted>");
                html = html.replace(&super::content::html(&value), "&lt;redacted&gt;");
                subject = subject.replace(&value, "<redacted>");
            }
        }
    }
    let mut builder = Message::builder()
        .from(
            a.from
                .parse()
                .map_err(|_| MailError::new("Absender ungültig"))?,
        )
        .subject(subject);
    let mut count = 0;
    for r in to.iter().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        builder = builder.to(r
            .parse()
            .map_err(|_| MailError::new("Empfängeradresse ungültig"))?);
        count += 1;
    }
    if count == 0 {
        return Ok(());
    }
    let message = builder
        .multipart(
            MultiPart::alternative()
                .singlepart(SinglePart::plain(text))
                .singlepart(SinglePart::html(html)),
        )
        .map_err(|_| MailError::new("Mail-Inhalt ungültig"))?;
    let mut stream = tunnel(a, n, secrets).await?;
    if a.tls == Tls::Tls {
        let peer = stream.peer;
        stream = Stream {
            io: Box::new(
                os_fetch::net::tls_tunnel(stream, &a.host, n)
                    .await
                    .map_err(MailError::new)?,
            ),
            peer,
        };
    }
    let hello = ClientId::Domain("openspider.local".into());
    let mut connection = AsyncSmtpConnection::connect_with_transport(Box::new(stream), &hello)
        .await
        .map_err(smtp)?;
    if a.tls == Tls::Starttls {
        let mut tls = lettre::transport::smtp::client::TlsParameters::builder(a.host.clone());
        for cert in os_fetch::net::certificates::roots(n).map_err(MailError::new)? {
            tls = tls.add_root_certificate(
                lettre::transport::smtp::client::Certificate::from_der(cert.to_vec())
                    .map_err(smtp)?,
            );
        }
        connection
            .starttls(tls.build().map_err(smtp)?, &hello)
            .await
            .map_err(smtp)?;
    }
    if !a.username.is_empty() {
        let mut secret = secrets
            .try_get(&format!("mail/{}", a.id))
            .map_err(|_| MailError::new("Mail-Schlüsselbund nicht lesbar"))?
            .ok_or_else(|| MailError::new("SMTP-Kennwort oder OAuth-Token fehlt"))?;
        let mechanisms = if a.auth == Auth::Password {
            vec![Mechanism::Plain, Mechanism::Login]
        } else {
            let mut token: serde_json::Value = serde_json::from_str(&secret)
                .map_err(|_| MailError::new("OAuth-Verbindung erneut autorisieren"))?;
            let expired = token["expires_at"]
                .as_str()
                .and_then(|t| os_model::Timestamp::from_db(t).ok())
                .is_none_or(|t| t <= os_model::SystemClock.now());
            if expired {
                use os_model::Clock;
                let client = a
                    .client_id
                    .as_deref()
                    .ok_or_else(|| MailError::new("OAuth-Client-ID fehlt"))?;
                let refresh = token["refresh_token"]
                    .as_str()
                    .ok_or_else(|| MailError::new("OAuth erneut autorisieren"))?;
                let endpoint = if a.auth == Auth::Gmail {
                    "https://oauth2.googleapis.com/token"
                } else {
                    "https://login.microsoftonline.com/common/oauth2/v2.0/token"
                };
                let proxy_pass = secrets.get(os_fetch::net::PROXY_PASSWORD);
                let mut form = vec![
                    ("grant_type", "refresh_token"),
                    ("client_id", client),
                    ("refresh_token", refresh),
                ];
                if let Some(client_secret) =
                    token["client_secret"].as_str().filter(|s| !s.is_empty())
                {
                    form.push(("client_secret", client_secret));
                }
                let response =
                    os_fetch::net::mail_token_response(n, proxy_pass.as_deref(), endpoint, &form)
                        .await
                        .map_err(MailError::new)?;
                let value: serde_json::Value = serde_json::from_str(&response)
                    .map_err(|_| MailError::new("OAuth-Tokenantwort ungültig"))?;
                token["access_token"] = value["access_token"].clone();
                if value["refresh_token"].is_string() {
                    token["refresh_token"] = value["refresh_token"].clone();
                }
                token["expires_at"] = serde_json::json!(os_model::SystemClock
                    .now()
                    .plus(Duration::from_secs(
                        value["expires_in"].as_u64().unwrap_or(3600)
                    ))
                    .to_db());
                secrets
                    .set(&format!("mail/{}", a.id), &token.to_string())
                    .map_err(|_| MailError::new("OAuth-Token nicht speicherbar"))?;
            }
            secret = token["access_token"]
                .as_str()
                .ok_or_else(|| MailError::new("OAuth-Zugriffstoken fehlt"))?
                .into();
            vec![Mechanism::Xoauth2]
        };
        connection
            .auth(&mechanisms, &Credentials::new(a.username.clone(), secret))
            .await
            .map_err(smtp)?;
    }
    connection
        .send(message.envelope(), &message.formatted())
        .await
        .map_err(smtp)?;
    let _ = connection.quit().await;
    Ok(())
}
use os_model::Clock;

#[cfg(test)]
mod tests {
    // spec: CRAWL-122 [M2]
    // spec: OUT-269
    #[test]
    fn smtp_bypass_matches_domain_boundaries_and_case_like_the_http_path() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/out-m2-mail/bypass.json"
        ))
        .unwrap();
        for case in cases.as_array().unwrap() {
            let patterns = vec![case["pattern"].as_str().unwrap().to_owned()];
            assert_eq!(
                super::bypass(case["host"].as_str().unwrap(), &patterns),
                case["bypass"].as_bool().unwrap(),
                "{case}"
            );
        }
    }
}
