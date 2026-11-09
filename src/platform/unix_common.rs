//! Pieces shared by the macOS and Linux backends.

use std::path::PathBuf;
use std::sync::OnceLock;

use super::{Clipboard, ClipboardBackup};

enum Saved {
    Text(String),
    Image { width: usize, height: usize, bytes: Vec<u8> },
}

/// Clipboard implemented with `arboard`. Backups keep text or images (other formats are lost).
#[derive(Default)]
pub struct ArboardClipboard;

impl ArboardClipboard {
    fn with<R>(f: impl FnOnce(&mut arboard::Clipboard) -> Option<R>) -> Option<R> {
        match arboard::Clipboard::new() {
            Ok(mut cb) => f(&mut cb),
            Err(e) => {
                log::warn!("Clipboard unavailable: {e}");
                None
            }
        }
    }
}

impl Clipboard for ArboardClipboard {
    fn text(&mut self) -> Option<String> {
        Self::with(|cb| cb.get_text().ok())
    }

    fn set_text(&mut self, text: &str) -> bool {
        // the snippet should not end up in clipboard history managers (Klipper, macOS clipboard managers...)
        #[cfg(target_os = "macos")]
        use arboard::SetExtApple as _;
        #[cfg(target_os = "linux")]
        use arboard::SetExtLinux as _;
        Self::with(|cb| cb.set().exclude_from_history().text(text.to_string()).ok()).is_some()
    }

    fn backup(&mut self) -> Option<ClipboardBackup> {
        Self::with(|cb| {
            if let Ok(t) = cb.get_text() {
                return Some(ClipboardBackup(Box::new(Saved::Text(t))));
            }
            if let Ok(img) = cb.get_image() {
                return Some(ClipboardBackup(Box::new(Saved::Image {
                    width: img.width,
                    height: img.height,
                    bytes: img.bytes.into_owned(),
                })));
            }
            None
        })
    }

    fn restore(&mut self, backup: ClipboardBackup) {
        let Ok(saved) = backup.0.downcast::<Saved>() else { return };
        Self::with(|cb| match *saved {
            Saved::Text(t) => cb.set_text(t).ok(),
            Saved::Image { width, height, bytes } => {
                cb.set_image(arboard::ImageData { width, height, bytes: std::borrow::Cow::Owned(bytes) }).ok()
            }
        });
    }

    fn clear(&mut self) {
        Self::with(|cb| cb.clear().ok());
    }
}

/// Write the built-in notification sound to a temporary file (once) and return its path.
pub fn notification_wav_file(bytes: &'static [u8]) -> Option<PathBuf> {
    static PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
    PATH.get_or_init(|| {
        let p = std::env::temp_dir().join(format!("expandercraft-notification-{}.wav", std::process::id()));
        std::fs::write(&p, bytes).ok().map(|_| p)
    })
    .clone()
}

/// Play a sound file with the first available command-line player.
pub fn play_with_commands(path: &str, players: &[&str]) {
    let path = path.to_string();
    let players: Vec<String> = players.iter().map(|s| s.to_string()).collect();
    std::thread::spawn(move || {
        for p in players {
            match std::process::Command::new(&p).arg(&path).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status()
            {
                Ok(s) if s.success() => return,
                _ => continue,
            }
        }
    });
}

pub fn current_exe_string() -> String {
    std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_else(|_| "expandercraft".into())
}

/// Returns false if another instance is already running (and asks it to show its main window).
pub fn acquire_single_instance(socket: &std::path::Path, on_show_request: impl Fn() + Send + 'static) -> bool {
    use std::io::Write;
    use std::os::unix::net::{UnixListener, UnixStream};
    if let Ok(mut s) = UnixStream::connect(socket) {
        let _ = s.write_all(b"show");
        return false;
    }
    let _ = std::fs::remove_file(socket);
    match UnixListener::bind(socket) {
        Ok(listener) => {
            std::thread::Builder::new()
                .name("expandercraft-instance".into())
                .spawn(move || {
                    for stream in listener.incoming() {
                        if stream.is_ok() {
                            on_show_request();
                        }
                    }
                })
                .ok();
        }
        Err(e) => log::warn!("Could not create the instance socket: {e}"),
    }
    true
}
