//! Appends lines to `tab-titles.log` in the plugin's state directory. The file
//! is cut back to empty once it grows past 1 MB.

use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

static PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
const MAX_BYTES: u64 = 1_000_000;

pub fn init(path: Option<PathBuf>) {
    let _ = PATH.set(path);
}

pub fn line(message: &str) {
    let Some(Some(path)) = PATH.get() else {
        eprintln!("{message}");
        return;
    };
    if std::fs::metadata(path)
        .map(|m| m.len() > MAX_BYTES)
        .unwrap_or(false)
    {
        let _ = std::fs::write(path, b"");
    }
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{secs} [{}] {message}", std::process::id());
    }
}
