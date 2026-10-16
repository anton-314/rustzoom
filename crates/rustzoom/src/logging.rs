use std::fs::{self, File};
use std::path::PathBuf;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

/// Logs to `%LOCALAPPDATA%\rustzoom\rustzoom.log` (overwritten on every start) and, in debug builds,
/// to stderr. Level via `RUSTZOOM_LOG` (default `info`). Keep the guard alive until exit.
pub fn init() -> Option<WorkerGuard> {
    let filter = EnvFilter::try_from_env("RUSTZOOM_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    let stderr = cfg!(debug_assertions).then(|| fmt::layer().with_writer(std::io::stderr));

    let (file, guard) = match open_log_file() {
        Some(file) => {
            let (writer, guard) = tracing_appender::non_blocking(file);
            (
                Some(fmt::layer().with_writer(writer).with_ansi(false)),
                Some(guard),
            )
        }
        None => (None, None),
    };

    tracing_subscriber::registry()
        .with(filter)
        .with(stderr)
        .with(file)
        .init();
    guard
}

fn open_log_file() -> Option<File> {
    let dir = PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("rustzoom");
    fs::create_dir_all(&dir).ok()?;
    File::create(dir.join("rustzoom.log")).ok()
}
