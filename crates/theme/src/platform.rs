//! What the renderer and the compositor will do behind our paint.

/// Whether this build has the backdrop-blur primitive behind it — the lens a
/// glass surface refracts through, and the frost a card lays over the content
/// it covers. Metal's and wgpu's; it tracks the gpui in use rather than the
/// platform, and the DirectX renderer carries no such primitive.
pub const LENSED: bool = cfg!(any(target_os = "macos", target_family = "wasm"));

/// Whether the compositor puts anything behind a translucent window — AppKit's
/// vibrancy, and Mica on Windows.
///
/// Read at runtime rather than from a `cfg`, because the Windows answer is a
/// build number: `DWMWA_SYSTEMBACKDROP_TYPE` lands in build 22621, and below it
/// gpui's backend applies no backdrop while its renderer still clears the
/// window transparent — the desktop then shows through unblurred. The build is
/// read once.
pub fn frosted_window() -> bool {
    #[cfg(target_os = "windows")]
    {
        static MICA: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *MICA.get_or_init(|| windows::build() >= windows::MICA_BUILD)
    }
    #[cfg(not(target_os = "windows"))]
    {
        cfg!(any(target_os = "macos", target_family = "wasm"))
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use windows_sys::{
        Wdk::System::SystemServices::RtlGetVersion,
        Win32::System::SystemInformation::OSVERSIONINFOW,
    };

    /// The build `DWMWA_SYSTEMBACKDROP_TYPE` lands in — Windows 11 22H2. gpui's
    /// backend reads the same number and returns without applying a backdrop
    /// below it.
    pub(super) const MICA_BUILD: u32 = 22621;

    /// The running build, or 0 where it cannot be read. `RtlGetVersion` rather
    /// than `GetVersionEx`, which reports 6.2 to a process whose manifest does
    /// not claim a later version.
    pub(super) fn build() -> u32 {
        let mut version = OSVERSIONINFOW {
            dwOSVersionInfoSize: size_of::<OSVERSIONINFOW>() as u32,
            ..Default::default()
        };
        // NTSTATUS: negative is a failure.
        if unsafe { RtlGetVersion(&mut version) } < 0 {
            return 0;
        }
        version.dwBuildNumber
    }
}
