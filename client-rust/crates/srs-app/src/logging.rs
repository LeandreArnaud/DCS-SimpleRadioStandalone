//! Logging to stderr and to `clientlog.txt` in the working directory
//! (the C# client's NLog target). The exe has no console on Windows in
//! release builds, so the file is the only trace the user can send back.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::Mutex;

struct FileLogger {
    file: Mutex<Option<File>>,
    level: log::LevelFilter,
}

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let line = format!(
            "{now} {:5} {} - {}\n",
            record.level(),
            record.target(),
            record.args()
        );
        let _ = std::io::stderr().write_all(line.as_bytes());
        if let Ok(mut f) = self.file.lock() {
            if let Some(f) = f.as_mut() {
                let _ = f.write_all(line.as_bytes());
            }
        }
    }

    fn flush(&self) {}
}

/// `RUST_LOG=debug` raises the level; default info. Rotates `clientlog.txt`
/// to `clientlog.old.txt` above 100 MB like NLog.
pub fn init() {
    let level = match std::env::var("RUST_LOG").ok().as_deref() {
        Some("trace") => log::LevelFilter::Trace,
        Some("debug") => log::LevelFilter::Debug,
        Some("warn") => log::LevelFilter::Warn,
        Some("error") => log::LevelFilter::Error,
        _ => log::LevelFilter::Info,
    };
    if std::fs::metadata("clientlog.txt")
        .map(|m| m.len() > 104_857_600)
        .unwrap_or(false)
    {
        let _ = std::fs::rename("clientlog.txt", "clientlog.old.txt");
    }
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("clientlog.txt")
        .ok();
    let logger = FileLogger {
        file: Mutex::new(file),
        level,
    };
    if log::set_boxed_logger(Box::new(logger)).is_ok() {
        log::set_max_level(level);
    }
}
