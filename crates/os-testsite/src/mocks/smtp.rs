//! Loopback SMTP fixture with authentication, TLS and captured MIME messages (OUT-280).
use base64::{engine::general_purpose::STANDARD, Engine};
use std::{
    io,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Security {
    Plain,
    Starttls,
    Tls,
}
#[derive(Clone)]
pub struct Config {
    pub username: String,
    pub password: String,
    pub security: Security,
    pub reject_data: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            username: "crawler".into(),
            password: "fixture-password".into(),
            security: Security::Plain,
            reject_data: false,
        }
    }
}
#[derive(Debug, Clone)]
pub struct Captured {
    pub from: String,
    pub to: Vec<String>,
    pub data: String,
}
pub struct SmtpMock {
    pub addr: SocketAddr,
    pub ca_pem: String,
    messages: Arc<Mutex<Vec<Captured>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for SmtpMock {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl SmtpMock {
    pub async fn start(config: Config) -> io::Result<Self> {
        let material = crate::tls::TlsMaterial::new()?;
        let ca_pem = material.ca_pem().to_owned();
        let acceptor = tokio_rustls::TlsAcceptor::from(material.server_config);
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
        let addr = listener.local_addr()?;
        let messages = Arc::new(Mutex::new(vec![]));
        let captured = messages.clone();
        let task = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let c = config.clone();
                let m = captured.clone();
                let tls = acceptor.clone();
                tokio::spawn(async move {
                    let io: Box<dyn Io> = if c.security == Security::Tls {
                        match tls.accept(stream).await {
                            Ok(s) => Box::new(s),
                            Err(_) => return,
                        }
                    } else {
                        Box::new(stream)
                    };
                    let _ = serve(io, c, m, tls).await;
                });
            }
        });
        Ok(Self {
            addr,
            ca_pem,
            messages,
            task,
        })
    }
    pub fn messages(&self) -> Vec<Captured> {
        self.messages
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
}
trait Io: AsyncRead + AsyncWrite + Send + Unpin {}
impl<T: AsyncRead + AsyncWrite + Send + Unpin> Io for T {}
async fn serve(
    io: Box<dyn Io>,
    config: Config,
    messages: Arc<Mutex<Vec<Captured>>>,
    tls: tokio_rustls::TlsAcceptor,
) -> io::Result<()> {
    let mut s = BufReader::new(io);
    s.write_all(b"220 localhost fixture SMTP\r\n").await?;
    let mut encrypted = config.security == Security::Tls;
    let mut authenticated = config.username.is_empty();
    let mut from = String::new();
    let mut to = vec![];
    let mut login = 0;
    let mut login_user = String::new();
    loop {
        let mut line = String::new();
        if s.read_line(&mut line).await? == 0 {
            return Ok(());
        }
        if line.len() > 65536 {
            return Err(io::Error::other("line too large"));
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if login > 0 {
            let decoded = STANDARD.decode(line).unwrap_or_default();
            if login == 1 {
                login_user = String::from_utf8_lossy(&decoded).into_owned();
                login = 2;
                s.write_all(b"334 UGFzc3dvcmQ6\r\n").await?;
                continue;
            }
            authenticated = login_user == config.username && decoded == config.password.as_bytes();
            login = 0;
            s.write_all(if authenticated {
                b"235 2.7.0 Authenticated\r\n"
            } else {
                b"535 5.7.8 Authentication failed\r\n"
            })
            .await?;
            continue;
        }
        let upper = line.to_ascii_uppercase();
        if upper.starts_with("EHLO") {
            s.write_all(b"250-localhost\r\n250-8BITMIME\r\n250-SMTPUTF8\r\n250-AUTH PLAIN LOGIN XOAUTH2\r\n").await?;
            s.write_all(if config.security == Security::Starttls && !encrypted {
                b"250 STARTTLS\r\n"
            } else {
                b"250 OK\r\n"
            })
            .await?;
        } else if upper == "STARTTLS" {
            if encrypted || config.security != Security::Starttls {
                s.write_all(b"454 TLS unavailable\r\n").await?;
                continue;
            }
            s.write_all(b"220 Begin TLS\r\n").await?;
            s.flush().await?;
            let upgraded = tls.accept(s.into_inner()).await?;
            s = BufReader::new(Box::new(upgraded));
            encrypted = true;
            authenticated = config.username.is_empty();
        } else if upper.starts_with("AUTH LOGIN") {
            login = 1;
            s.write_all(b"334 VXNlcm5hbWU6\r\n").await?;
        } else if upper.starts_with("AUTH ") {
            let raw = line
                .split_whitespace()
                .nth(2)
                .and_then(|v| STANDARD.decode(v).ok())
                .unwrap_or_default();
            if upper.starts_with("AUTH PLAIN") {
                let p = raw.split(|b| *b == 0).collect::<Vec<_>>();
                authenticated = p.len() == 3
                    && p[1] == config.username.as_bytes()
                    && p[2] == config.password.as_bytes();
            } else if upper.starts_with("AUTH XOAUTH2") {
                authenticated = raw
                    == format!(
                        "user={}\x01auth=Bearer {}\x01\x01",
                        config.username, config.password
                    )
                    .as_bytes();
            }
            s.write_all(if authenticated {
                b"235 2.7.0 Authenticated\r\n"
            } else {
                b"535 5.7.8 Authentication failed\r\n"
            })
            .await?;
        } else if upper.starts_with("MAIL FROM:") {
            if !authenticated {
                s.write_all(b"530 Authentication required\r\n").await?;
                continue;
            }
            from = line[10..].into();
            to.clear();
            s.write_all(b"250 OK\r\n").await?;
        } else if upper.starts_with("RCPT TO:") {
            to.push(line[8..].into());
            s.write_all(b"250 OK\r\n").await?;
        } else if upper == "DATA" {
            if !authenticated {
                s.write_all(b"530 Authentication required\r\n").await?;
                continue;
            }
            if config.reject_data {
                s.write_all(b"554 Message rejected\r\n").await?;
                continue;
            }
            s.write_all(b"354 Send message\r\n").await?;
            let mut data = String::new();
            loop {
                let mut l = String::new();
                if s.read_line(&mut l).await? == 0 {
                    return Ok(());
                }
                if l == ".\r\n" {
                    break;
                }
                if let Some(rest) = l.strip_prefix("..") {
                    data.push('.');
                    data.push_str(rest);
                } else {
                    data.push_str(&l);
                }
                if data.len() > 2 * 1024 * 1024 {
                    return Err(io::Error::other("message too large"));
                }
            }
            messages
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(Captured {
                    from: from.clone(),
                    to: to.clone(),
                    data,
                });
            s.write_all(b"250 Queued\r\n").await?;
        } else if upper == "QUIT" {
            s.write_all(b"221 Bye\r\n").await?;
            return Ok(());
        } else {
            s.write_all(b"250 OK\r\n").await?;
        }
    }
}
