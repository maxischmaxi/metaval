//! Kleine Lade-Animation (Spinner) auf **stderr** während Netzwerk-Wartezeiten.
//!
//! Bewusst nur auf stderr: stdout (pretty-Report/JSON) bleibt komplett unberührt,
//! der Spinner stört also kein `| jq` und keine Datei-Umleitung. Aktiv nur, wenn
//! stderr ein Terminal ist und der Aufrufer es erlaubt (kein `--no-color`/`NO_COLOR`,
//! kein `-v`). Keine zusätzliche Dependency: `IsTerminal` + tokio-Timer + Atomic.

use std::io::{IsTerminal, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::task::JoinHandle;

const FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const INTERVAL: Duration = Duration::from_millis(80);

/// Laufender Spinner. Mit [`Spinner::finish`] sauber stoppen; `Drop` ist ein
/// Sicherheitsnetz, falls `finish` nicht erreicht wird (z. B. früher `?`-Return).
pub struct Spinner {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Spinner {
    /// Startet den Spinner mit `message`, sofern `enabled` und stderr interaktiv ist.
    /// Andernfalls ein No-Op-Spinner (keine Ausgabe, kein Task).
    pub fn start(message: impl Into<String>, enabled: bool) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        if !enabled || !std::io::stderr().is_terminal() {
            return Self { stop, handle: None };
        }

        let message = message.into();
        let flag = stop.clone();
        let handle = tokio::spawn(async move {
            let mut frame = 0usize;
            while !flag.load(Ordering::Relaxed) {
                {
                    let mut err = std::io::stderr().lock();
                    let _ = write!(err, "\r{} {} ", FRAMES[frame % FRAMES.len()], message);
                    let _ = err.flush();
                }
                frame += 1;
                tokio::time::sleep(INTERVAL).await;
            }
        });
        Self { stop, handle: Some(handle) }
    }

    /// Stoppt den Spinner und räumt die Zeile auf.
    pub async fn finish(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.await;
            clear_line();
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            handle.abort();
            clear_line();
        }
    }
}

/// Löscht die aktuelle Terminal-Zeile (Cursor an Anfang + Zeile leeren).
fn clear_line() {
    let mut err = std::io::stderr().lock();
    let _ = write!(err, "\r\x1b[2K");
    let _ = err.flush();
}
