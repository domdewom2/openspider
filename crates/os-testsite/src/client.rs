//! Minimaler HTTP/1.1-Client mit Keep-Alive für Tests und Lastmessungen.
//!
//! Absichtlich ohne Abhängigkeit auf einen HTTP-Stack: Er spricht nur mit `os-testsite`
//! (Antworten mit `Content-Length`), folgt keinen Weiterleitungen und liefert Header und
//! Body unverändert zurück.

use std::io;
use std::net::SocketAddr;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

/// Antwort auf einen GET-Request.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// HTTP-Statuscode.
    pub status: u16,
    /// Header (Namen kleingeschrieben).
    pub headers: Vec<(String, String)>,
    /// Body-Bytes.
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// Wert eines Headers (Name kleingeschrieben angeben).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// Body als (verlustbehafteter) UTF-8-Text.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// Eine persistente Verbindung (Keep-Alive).
pub struct HttpClient {
    reader: BufReader<TcpStream>,
    host: String,
}

impl HttpClient {
    /// Verbindet zu `addr`.
    pub async fn connect(addr: SocketAddr) -> io::Result<Self> {
        let stream = TcpStream::connect(addr).await?;
        stream.set_nodelay(true)?;
        Ok(Self {
            reader: BufReader::new(stream),
            host: addr.to_string(),
        })
    }

    /// Sendet `GET <path>` und liest die vollständige Antwort.
    pub async fn get(&mut self, path: &str) -> io::Result<HttpResponse> {
        let request = format!(
            "GET {path} HTTP/1.1\r\nHost: {}\r\nAccept: */*\r\n\r\n",
            self.host
        );
        self.reader.get_mut().write_all(request.as_bytes()).await?;

        let mut line = String::new();
        self.reader.read_line(&mut line).await?;
        let status: u16 = line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| invalid(format!("ungültige Statuszeile: {line:?}")))?;

        let mut headers = Vec::new();
        loop {
            line.clear();
            if self.reader.read_line(&mut line).await? == 0 {
                return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
            }
            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                break;
            }
            if let Some((k, v)) = trimmed.split_once(':') {
                headers.push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
            }
        }

        let len: usize = headers
            .iter()
            .find(|(k, _)| k == "content-length")
            .and_then(|(_, v)| v.parse().ok())
            .ok_or_else(|| invalid("Antwort ohne Content-Length".into()))?;
        let mut body = vec![0u8; len];
        self.reader.read_exact(&mut body).await?;
        Ok(HttpResponse {
            status,
            headers,
            body,
        })
    }
}

/// Einmaliger GET auf einer frischen Verbindung.
pub async fn get_once(addr: SocketAddr, path: &str) -> io::Result<HttpResponse> {
    HttpClient::connect(addr).await?.get(path).await
}

fn invalid(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}
