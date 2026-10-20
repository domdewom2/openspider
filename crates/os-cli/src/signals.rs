//! Abbruch per Signal (OUT-206, CRAWL-126).
//!
//! * Erstes SIGINT/SIGTERM (macOS) bzw. Strg+C/Strg+Untbr/Konsole schließen (Windows): der Crawl wird
//!   pausiert und gespeichert, die Analyse abgebrochen, es folgen keine Exporte; Exit-Code 130.
//! * Zweites Signal: sofortiger Abbruch ohne Abwarten laufender Anfragen; die Crawl-Datei bleibt dank WAL
//!   öffnenbar und lässt sich mit `--load-crawl <id> --resume` fortsetzen.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use os_analyze::analysis::CancelToken;
use tokio::sync::watch;

use crate::exit::Exit;
use crate::reporter::Reporter;

/// Zustand der Signalbehandlung.
#[derive(Debug, Clone)]
pub struct Signals {
    interrupted: Arc<AtomicBool>,
    first: watch::Receiver<bool>,
    first_tx: Arc<watch::Sender<bool>>,
    cancel: CancelToken,
}

impl Signals {
    /// Installiert die Handler (im Kontext einer tokio-Laufzeit). `cancel` wird beim ersten Signal
    /// ausgelöst, damit eine laufende Analyse abbricht.
    pub fn install(cancel: CancelToken, rep: Arc<Reporter>) -> std::io::Result<Signals> {
        let interrupted = Arc::new(AtomicBool::new(false));
        let (tx, rx) = watch::channel(false);
        let tx = Arc::new(tx);
        let mut source = Source::new()?;
        let first_tx = Arc::clone(&tx);
        let flag = Arc::clone(&interrupted);
        let token = cancel.clone();
        tokio::spawn(async move {
            source.recv().await;
            flag.store(true, Ordering::SeqCst);
            token.cancel();
            let _ = tx.send(true);
            rep.info(
                "Unterbrechung – der Crawl wird pausiert und gespeichert \
                 (ein zweites Signal beendet sofort)",
            );
            source.recv().await;
            rep.info(format!(
                "openspider: Sofortiger Abbruch durch zweites Signal; die Crawl-Datei bleibt \
                 öffnenbar (Exit-Code {})",
                Exit::Interrupted.code()
            ));
            std::process::exit(Exit::Interrupted.code());
        });
        Ok(Signals {
            interrupted,
            first: rx,
            first_tx,
            cancel,
        })
    }

    /// Ohne Betriebssystem-Handler: Abbruch nur über [`Signals::request_stop`] (Tests, eingebettete Läufe).
    pub fn manual(cancel: CancelToken) -> Signals {
        let (tx, rx) = watch::channel(false);
        Signals {
            interrupted: Arc::new(AtomicBool::new(false)),
            first: rx,
            first_tx: Arc::new(tx),
            cancel,
        }
    }

    /// Löst den ersten Abbruch ohne Signal aus, wie ein erstes SIGINT: Der Crawl wird pausiert und
    /// gespeichert, es folgen keine Exporte (Abbruchmarke geplanter Läufe, OUT-171).
    pub fn request_stop(&self) {
        if !self.interrupted.swap(true, Ordering::SeqCst) {
            self.cancel.cancel();
            let _ = self.first_tx.send(true);
        }
    }

    /// Kam schon ein Signal?
    pub fn is_interrupted(&self) -> bool {
        self.interrupted.load(Ordering::SeqCst)
    }

    /// Wird `true`, sobald das erste Signal eintrifft.
    pub fn first_signal(&self) -> watch::Receiver<bool> {
        self.first.clone()
    }

    /// Abbruchmarke der Analyse.
    pub fn cancel_token(&self) -> CancelToken {
        self.cancel.clone()
    }
}

#[cfg(unix)]
struct Source {
    int: tokio::signal::unix::Signal,
    term: tokio::signal::unix::Signal,
}

#[cfg(unix)]
impl Source {
    fn new() -> std::io::Result<Self> {
        use tokio::signal::unix::{signal, SignalKind};
        Ok(Source {
            int: signal(SignalKind::interrupt())?,
            term: signal(SignalKind::terminate())?,
        })
    }

    async fn recv(&mut self) {
        tokio::select! {
            _ = self.int.recv() => {}
            _ = self.term.recv() => {}
        }
    }
}

#[cfg(windows)]
struct Source {
    ctrl_c: tokio::signal::windows::CtrlC,
    ctrl_break: tokio::signal::windows::CtrlBreak,
    ctrl_close: tokio::signal::windows::CtrlClose,
}

#[cfg(windows)]
impl Source {
    fn new() -> std::io::Result<Self> {
        Ok(Source {
            ctrl_c: tokio::signal::windows::ctrl_c()?,
            ctrl_break: tokio::signal::windows::ctrl_break()?,
            ctrl_close: tokio::signal::windows::ctrl_close()?,
        })
    }

    async fn recv(&mut self) {
        tokio::select! {
            _ = self.ctrl_c.recv() => {}
            _ = self.ctrl_break.recv() => {}
            _ = self.ctrl_close.recv() => {}
        }
    }
}
