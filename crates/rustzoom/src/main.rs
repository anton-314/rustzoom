#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod capture;
#[cfg(windows)]
mod engine;
#[cfg(windows)]
mod gui;
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
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!("{err:#}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(windows)]
fn run() -> anyhow::Result<()> {
    use anyhow::Context;
    use windows::Win32::UI::HiDpi::{
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
    };

    // Physical pixels everywhere; otherwise scope and capture are misaligned at 125 %/150 % scaling.
    // SAFETY: called once at startup, before any window exists.
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) }
        .context("SetProcessDpiAwarenessContext")?;

    let path = config_path().context("%APPDATA% is not set")?;
    let loaded = rustzoom_core::config::load(&path);
    if let Some(warning) = &loaded.warning {
        tracing::warn!("{warning}");
    }
    tracing::info!(
        "rustzoom {} started, config {}",
        env!("CARGO_PKG_VERSION"),
        path.display()
    );

    gui::run(loaded.config, path)?;
    tracing::info!("rustzoom exiting");
    Ok(())
}

/// `%APPDATA%\rustzoom\config.toml`
#[cfg(windows)]
fn config_path() -> Option<std::path::PathBuf> {
    Some(
        std::path::PathBuf::from(std::env::var_os("APPDATA")?)
            .join("rustzoom")
            .join("config.toml"),
    )
}

#[cfg(not(windows))]
fn main() {
    eprintln!("rustzoom only runs on Windows 11.");
    std::process::exit(1);
}
