//! Single-instance guard.

use anyhow::{Context, Result};
use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::w;

/// Named mutex held for the process lifetime; a second instance sees it and exits.
pub struct SingleInstance(HANDLE);

impl SingleInstance {
    pub fn acquire() -> Result<Option<Self>> {
        // SAFETY: creating/opening a named mutex with default security.
        let handle = unsafe { CreateMutexW(None, true, w!("Local\\rustzoom-single-instance")) }
            .context("CreateMutexW")?;
        // SAFETY: reads the error code CreateMutexW just set.
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            // SAFETY: we own this handle and don't use it afterwards.
            let _ = unsafe { CloseHandle(handle) };
            return Ok(None);
        }
        Ok(Some(Self(handle)))
    }
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        // SAFETY: we own the handle.
        let _ = unsafe { CloseHandle(self.0) };
    }
}
