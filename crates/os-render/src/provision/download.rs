//! Download eines Chromium-Archivs: fortsetzbar (HTTP-Range), mit Obergrenze, Abbruch und Prüfsumme vor dem Entpacken.
//!
//! Der Transport kommt vom Aufrufer; die App verwendet ihren HTTP-Client mit dem app-weiten Proxy (OUT-269,
//! CRAWL-122). So bleibt dieses Crate ohne eigenen HTTP-Stack.
use std::{
    fs,
    io::Write,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use futures_util::{future::BoxFuture, stream::BoxStream, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

use super::{
    install::{self, Installed},
    Plan, ProvisionError,
};

/// Ein Stück des Antwortkörpers oder ein Übertragungsfehler.
pub type Chunk = Result<Vec<u8>, String>;

/// Antwort auf eine GET-Anfrage.
pub struct Response {
    pub status: u16,
    /// Wert des Headers `Content-Range` (bei 206).
    pub content_range: Option<String>,
    pub body: BoxStream<'static, Chunk>,
}

/// HTTP-Zugang für den Download. `offset > 0` verlangt `Range: bytes=<offset>-`.
pub trait Transport: Send + Sync {
    fn get<'a>(&'a self, url: &'a str, offset: u64) -> BoxFuture<'a, Result<Response, String>>;
}

/// Abbruchmarke; der Download bricht auch während einer stockenden Übertragung sofort ab.
#[derive(Debug, Clone, Default)]
pub struct Cancel(Arc<(AtomicBool, Notify)>);

impl Cancel {
    pub fn new() -> Cancel {
        Cancel::default()
    }
    pub fn cancel(&self) {
        self.0 .0.store(true, Ordering::SeqCst);
        self.0 .1.notify_waiters();
    }
    pub fn is_cancelled(&self) -> bool {
        self.0 .0.load(Ordering::SeqCst)
    }
    async fn cancelled(&self) {
        loop {
            let notified = self.0 .1.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.is_cancelled() {
                return;
            }
            notified.await;
        }
    }
}

/// Abschnitt der Bereitstellung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Downloading,
    Verifying,
    Installing,
}

/// Fortschritt in Byte (`total` = erwartete Archivgröße).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub phase: Phase,
    pub done: u64,
    pub total: u64,
}

/// Lädt, prüft und installiert das Archiv eines Plans unter `root`.
///
/// - Ein vorhandener Teil-Download wird mit `Range` fortgesetzt; antwortet der Server mit 200, beginnt er neu.
/// - Abbruch ([`Cancel`]) und Übertragungsfehler lassen den Teil-Download für eine Fortsetzung liegen.
/// - Mehr Bytes als angekündigt, falsche Prüfsumme oder ein ungültiges Archiv verwerfen ihn; es wird nichts entpackt.
pub async fn provision(
    plan: &Plan,
    root: &Path,
    transport: &dyn Transport,
    cancel: &Cancel,
    progress: &(dyn Fn(Progress) + Send + Sync),
) -> Result<Installed, ProvisionError> {
    let part = install::partial_path(root, plan);
    let dir = part.parent().expect("partial dir");
    fs::create_dir_all(dir)?;
    // Teil-Downloads anderer Versionen/Plattformen sind wertlos.
    for entry in fs::read_dir(dir)?.flatten() {
        if entry.path() != part {
            let _ = fs::remove_file(entry.path());
        }
    }
    let total = plan.archive.size;
    let mut done = fs::metadata(&part).map_or(0, |m| m.len());
    if done > total {
        fs::remove_file(&part)?;
        done = 0;
    }
    if done < total {
        done = fetch(plan, &part, done, transport, cancel, progress).await?;
    }
    if done != total {
        return Err(ProvisionError::Network(format!(
            "unvollständig ({done} von {total} Byte)"
        )));
    }
    if cancel.is_cancelled() {
        return Err(ProvisionError::Cancelled);
    }
    progress(Progress {
        phase: Phase::Verifying,
        done,
        total,
    });
    let actual = blocking({
        let part = part.clone();
        move || install::sha256_file(&part).map_err(ProvisionError::from)
    })
    .await?;
    if actual != plan.archive.sha256 {
        let _ = fs::remove_file(&part);
        return Err(ProvisionError::Checksum {
            expected: plan.archive.sha256.clone(),
            actual,
        });
    }
    progress(Progress {
        phase: Phase::Installing,
        done,
        total,
    });
    let result = blocking({
        let (part, root, plan) = (part.clone(), root.to_path_buf(), plan.clone());
        move || install::unpack_and_publish(&part, &root, &plan)
    })
    .await;
    // Bestätigtes Archiv wird nach dem Entpacken (oder bei ungültigem Inhalt) nicht mehr gebraucht.
    if !matches!(
        result,
        Err(ProvisionError::Io(_)) | Err(ProvisionError::InUse(_))
    ) {
        let _ = fs::remove_file(&part);
    }
    result
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, ProvisionError> + Send + 'static,
) -> Result<T, ProvisionError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| ProvisionError::Io(e.to_string()))?
}

/// Überträgt ab `offset` in die Teildatei; liefert die Länge der Teildatei.
async fn fetch(
    plan: &Plan,
    part: &Path,
    offset: u64,
    transport: &dyn Transport,
    cancel: &Cancel,
    progress: &(dyn Fn(Progress) + Send + Sync),
) -> Result<u64, ProvisionError> {
    let total = plan.archive.size;
    let response = tokio::select! {
        _ = cancel.cancelled() => return Err(ProvisionError::Cancelled),
        r = transport.get(&plan.archive.url, offset) => r.map_err(ProvisionError::Network)?,
    };
    let (mut file, mut done) = match response.status {
        200 => (fs::File::create(part)?, 0),
        206 if offset > 0
            && response
                .content_range
                .as_deref()
                .is_some_and(|r| r.trim().starts_with(&format!("bytes {offset}-"))) =>
        {
            (fs::OpenOptions::new().append(true).open(part)?, offset)
        }
        206 => {
            // Unpassender Bereich: Teil-Download ist nicht verlässlich fortsetzbar.
            let _ = fs::remove_file(part);
            return Err(ProvisionError::Network(format!(
                "unerwarteter Bereich {:?}",
                response.content_range
            )));
        }
        416 => {
            let _ = fs::remove_file(part);
            return Err(ProvisionError::Http(416));
        }
        status => return Err(ProvisionError::Http(status)),
    };
    progress(Progress {
        phase: Phase::Downloading,
        done,
        total,
    });
    let mut body = response.body;
    loop {
        let next = tokio::select! {
            _ = cancel.cancelled() => None,
            chunk = body.next() => Some(chunk),
        };
        let chunk = match next {
            None => {
                file.flush()?;
                return Err(ProvisionError::Cancelled);
            }
            Some(None) => break,
            Some(Some(Ok(chunk))) => chunk,
            Some(Some(Err(e))) => {
                file.flush()?;
                return Err(ProvisionError::Network(e));
            }
        };
        let actual = done + chunk.len() as u64;
        if actual > total {
            drop(file);
            let _ = fs::remove_file(part);
            return Err(ProvisionError::TooLarge {
                expected: total,
                actual,
            });
        }
        file.write_all(&chunk)?;
        done = actual;
        progress(Progress {
            phase: Phase::Downloading,
            done,
            total,
        });
    }
    file.flush()?;
    Ok(done)
}
