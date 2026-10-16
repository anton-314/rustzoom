#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod app;
#[cfg(windows)]
mod input;
#[cfg(windows)]
mod instance;
#[cfg(windows)]
mod logging;
#[cfg(windows)]
mod overlay;
#[cfg(windows)]
mod render;

#[cfg(windows)]
fn main() -> std::process::ExitCode {
    // Checked before logging starts, so a second instance doesn't truncate the running one's log.
    let _instance = match instance::SingleInstance::acquire() {
        Ok(Some(instance)) => instance,
        Ok(None) => return std::process::ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err:#}");
            return std::process::ExitCode::FAILURE;
        }
    };
    // Returning (instead of process::exit) lets the guard flush the log on errors.
    let _log_guard = logging::init();
    match app::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!("{err:#}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("rustzoom only runs on Windows 11.");
    std::process::exit(1);
}
