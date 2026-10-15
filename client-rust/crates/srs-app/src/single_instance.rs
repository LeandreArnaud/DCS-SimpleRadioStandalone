//! Single-instance guard (`App.IsClientRunning`): a named mutex
//! `DCS-SimpleRadioStandalone`. Off Windows there is no check.

/// Holds the mutex for the process lifetime.
pub struct InstanceGuard {
    #[cfg(windows)]
    _handle: windows_sys::Win32::Foundation::HANDLE,
}

/// Returns `Ok(guard)` when this is the only instance, `Err(())` when
/// another one already holds the mutex.
pub fn acquire() -> Result<InstanceGuard, ()> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
        use windows_sys::Win32::System::Threading::CreateMutexW;
        let name: Vec<u16> = "DCS-SimpleRadioStandalone\0".encode_utf16().collect();
        // SAFETY: name is a valid NUL-terminated UTF-16 string.
        let handle = unsafe { CreateMutexW(std::ptr::null(), 1, name.as_ptr()) };
        if handle.is_null() {
            return Ok(InstanceGuard { _handle: handle });
        }
        // SAFETY: plain Win32 call.
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            return Err(());
        }
        Ok(InstanceGuard { _handle: handle })
    }
    #[cfg(not(windows))]
    {
        Ok(InstanceGuard {})
    }
}

/// `TaskDialog` shown by the C# client before quitting.
pub fn show_already_running() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
        let text: Vec<u16> =
            "Another instance of the SimpleRadio client is already running!\nThis one will now quit.\0"
                .encode_utf16()
                .collect();
        let caption: Vec<u16> = "Multiple SimpleRadio clients started!\0".encode_utf16().collect();
        // SAFETY: both strings are NUL-terminated UTF-16.
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                caption.as_ptr(),
                MB_OK | MB_ICONERROR,
            )
        };
    }
}
