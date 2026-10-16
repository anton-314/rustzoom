#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
fn main() {
    println!("rustzoom {}", env!("CARGO_PKG_VERSION"));
}

#[cfg(not(windows))]
fn main() {
    eprintln!("rustzoom only runs on Windows 11.");
    std::process::exit(1);
}
