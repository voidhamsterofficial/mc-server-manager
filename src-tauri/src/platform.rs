//! Small OS-specific helpers shared across modules.

/// Prevents a console window from flashing up on Windows whenever we spawn a
/// child process. No-op elsewhere.
#[cfg(windows)]
pub fn hide_console_window(command: &mut tokio::process::Command) {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
pub fn hide_console_window(_command: &mut tokio::process::Command) {}
