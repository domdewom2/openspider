//! Gemeinsame Test-Helfer: minimaler HTTP/1.1-Client (Klartext und TLS) mit Chunked-Dekodierung.
#![allow(dead_code)]

use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use os_testsite::TestSite;
use rustls::pki_types::ServerName;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

#[derive(Debug, Clone)]
pub struct Resp {
    pub status: u16,
    pub reason: String,
    /// Header mit Kleinbuchstaben-Namen in Eingangsreihenfolge.
    pub headers: Vec<(String, String)>,
    /// Body, bei `Transfer-Encoding: chunked` dekodiert.
    pub body: Vec<u8>,
    /// Rohe Bytes nach dem Kopf (vor Chunked-Dekodierung).
    pub raw_body: Vec<u8>,
}

impl Resp {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn header_all(&self, name: &str) -> Vec<&str> {
        self.headers
            .iter()
            .filter(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
            .collect()
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

pub fn request_bytes(host: &str, path: &str, extra: &[(&str, &str)]) -> Vec<u8> {
    let mut req = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n");
    for (k, v) in extra {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    req.into_bytes()
}

/// Liest bis EOF oder Fehler und liefert, was angekommen ist, plus den Abbruchgrund.
pub async fn read_all<S: AsyncRead + Unpin>(stream: &mut S) -> (Vec<u8>, Option<io::Error>) {
    let mut out = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        match tokio::time::timeout(Duration::from_secs(20), stream.read(&mut chunk)).await {
            Ok(Ok(0)) => return (out, None),
            Ok(Ok(n)) => out.extend_from_slice(&chunk[..n]),
            Ok(Err(e)) => return (out, Some(e)),
            Err(_) => return (out, Some(io::Error::from(io::ErrorKind::TimedOut))),
        }
    }
}

pub fn parse(raw: &[u8]) -> Option<Resp> {
    let end = raw.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = String::from_utf8_lossy(&raw[..end]).into_owned();
    let mut lines = head.split("\r\n");
    let status_line = lines.next()?;
    let mut parts = status_line.splitn(3, ' ');
    let _version = parts.next()?;
    let status: u16 = parts.next()?.parse().ok()?;
    let reason = parts.next().unwrap_or("").to_string();
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    let raw_body = raw[end + 4..].to_vec();
    let chunked = headers
        .iter()
        .any(|(k, v)| k == "transfer-encoding" && v.eq_ignore_ascii_case("chunked"));
    let body = if chunked {
        dechunk(&raw_body)?
    } else {
        raw_body.clone()
    };
    Some(Resp {
        status,
        reason,
        headers,
        body,
        raw_body,
    })
}

pub fn dechunk(mut raw: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let eol = raw.windows(2).position(|w| w == b"\r\n")?;
        let size = usize::from_str_radix(std::str::from_utf8(&raw[..eol]).ok()?.trim(), 16).ok()?;
        raw = &raw[eol + 2..];
        if size == 0 {
            return Some(out);
        }
        if raw.len() < size + 2 {
            return None;
        }
        out.extend_from_slice(&raw[..size]);
        raw = &raw[size + 2..];
    }
}

/// GET über eine frische Klartext-Verbindung zu `addr` mit beliebigem Host-Header.
pub async fn get_at(addr: SocketAddr, host: &str, path: &str, extra: &[(&str, &str)]) -> Resp {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(&request_bytes(host, path, extra))
        .await
        .unwrap();
    let (raw, err) = read_all(&mut stream).await;
    parse(&raw).unwrap_or_else(|| panic!("keine gültige Antwort ({err:?}): {raw:?}"))
}

pub async fn get(site: &TestSite, host: &str, path: &str) -> Resp {
    get_at(site.addr(), host, path, &[]).await
}

pub async fn tls_connect(
    site: &TestSite,
    addr: SocketAddr,
    sni: &str,
) -> io::Result<tokio_rustls::client::TlsStream<TcpStream>> {
    let connector = TlsConnector::from(site.tls_client_config()?);
    let tcp = TcpStream::connect(addr).await?;
    let name = ServerName::try_from(sni.to_string()).map_err(io::Error::other)?;
    connector.connect(name, tcp).await
}

/// HTTPS-GET auf dem Port der Site mit `sni` als Servername und Host-Header.
pub async fn get_tls(site: &TestSite, sni: &str, path: &str) -> io::Result<Resp> {
    let mut stream = tls_connect(site, site.addr(), sni).await?;
    stream
        .write_all(&request_bytes(sni, path, &[]))
        .await
        .unwrap();
    let (raw, _) = read_all(&mut stream).await;
    Ok(parse(&raw).expect("gültige TLS-Antwort"))
}

pub async fn send_and_read<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    host: &str,
    path: &str,
) -> (Option<Resp>, Vec<u8>, Option<io::Error>) {
    stream
        .write_all(&request_bytes(host, path, &[]))
        .await
        .unwrap();
    let (raw, err) = read_all(stream).await;
    (parse(&raw), raw, err)
}
