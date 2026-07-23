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

/// Stops Windows from showing a modal "the code execution cannot proceed"
/// box when a process we launch fails to start — a broken or half-unpacked
/// JDK, say, whose `java.exe` can't find `jli.dll`. Child processes inherit
/// this mode, so one call at startup covers every JVM we spawn: they fail
/// with an error exit code we can handle instead of hanging on a dialog the
/// user has to dismiss. No-op elsewhere.
#[cfg(windows)]
pub fn suppress_hard_error_dialogs() {
    /// `SEM_FAILCRITICALERRORS`: report the failure to the caller rather than
    /// popping the system error box.
    const SEM_FAILCRITICALERRORS: u32 = 0x0001;

    #[link(name = "kernel32")]
    extern "system" {
        fn SetErrorMode(mode: u32) -> u32;
    }

    // SAFETY: `SetErrorMode` takes a plain flags word, touches no memory we
    // own, and cannot fail — it only returns the previous mode, which we
    // deliberately discard because nothing here restores it.
    unsafe {
        SetErrorMode(SEM_FAILCRITICALERRORS);
    }
}

#[cfg(not(windows))]
pub fn suppress_hard_error_dialogs() {}

/// Ties a spawned server process to this app via a kill-on-close job object,
/// so Windows reaps it even if the app is force-killed (crashes, or the dev
/// watcher restarting a build). Without this, `kill_on_drop` never runs and
/// the server survives as an orphan holding the world's session lock.
#[cfg(windows)]
pub fn tie_child_to_app_lifetime(child: &tokio::process::Child) {
    use std::sync::OnceLock;

    static APP_JOB: OnceLock<Option<win32job::Job>> = OnceLock::new();

    let job = APP_JOB.get_or_init(|| {
        let created = create_kill_on_close_job();
        if let Err(error) = &created {
            log::warn!("could not create process job object: {error}");
        }
        created.ok()
    });

    let Some(job) = job else {
        return;
    };
    let Some(raw_handle) = child.raw_handle() else {
        return;
    };
    if let Err(error) = job.assign_process(raw_handle as isize) {
        log::warn!("could not assign server process to job object: {error}");
    }
}

#[cfg(windows)]
fn create_kill_on_close_job() -> Result<win32job::Job, win32job::JobError> {
    let job = win32job::Job::create()?;
    let mut limits = job.query_extended_limit_info()?;
    limits.limit_kill_on_job_close();
    job.set_extended_limit_info(&limits)?;
    Ok(job)
}

#[cfg(not(windows))]
pub fn tie_child_to_app_lifetime(_child: &tokio::process::Child) {}
