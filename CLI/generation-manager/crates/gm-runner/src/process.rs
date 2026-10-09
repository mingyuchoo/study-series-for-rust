use crate::error::{Error,
                   IoContext,
                   Result};
use gm_core::RunState;
#[cfg(target_os = "linux")]
use std::{fs::File,
          os::fd::{AsRawFd,
                   FromRawFd}};
use std::{io,
          process::Child};

pub(crate) fn identity(pid: i32) -> Option<String> {
    if pid <= 0 {
        return None;
    }
    start_identity(pid)
}

#[cfg(target_os = "linux")]
fn start_identity(pid: i32) -> Option<String> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let fields = stat.rsplit_once(") ")?.1;
    let start = fields.split_whitespace().nth(19)?;
    let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id").ok()?;
    Some(format!("{}:{start}", boot.trim()))
}

#[cfg(target_os = "macos")]
fn start_identity(pid: i32) -> Option<String> {
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::uninit();
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as i32;
    let read = unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDTBSDINFO, 0, info.as_mut_ptr().cast(), size) };
    if read != size {
        return None;
    }
    let info = unsafe { info.assume_init() };
    Some(format!("{}:{}", info.pbi_start_tvsec, info.pbi_start_tvusec))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn start_identity(_pid: i32) -> Option<String> { None }

pub(crate) fn alive(pid: i32) -> bool {
    if pid <= 0 || unsafe { libc::kill(pid, 0) } != 0 {
        return false;
    }
    #[cfg(target_os = "linux")]
    {
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
            return false;
        };
        stat.rsplit_once(") ").is_some_and(|(_, fields)| !fields.starts_with("Z "))
    }
    #[cfg(not(target_os = "linux"))]
    true
}

pub(crate) fn matches(state: &RunState) -> bool { state.process_start.is_some() && identity(state.pid) == state.process_start }

/// The caller still owns this unreaped child, so its PID cannot be reused.
pub(crate) fn terminate(child: &mut Child) -> io::Result<()> {
    if unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) } == -1 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::ESRCH) {
            return Err(error);
        }
    }
    child.wait()?;
    Ok(())
}

/// Keeps a stable kernel reference while checking and signaling a Linux
/// process.
pub(crate) struct Process {
    #[cfg(target_os = "linux")]
    fd: File,
    #[cfg(not(target_os = "linux"))]
    state: RunState,
}

impl Process {
    pub(crate) fn open(state: &RunState) -> Result<Self> {
        if state.pid <= 0 || state.process_start.is_none() {
            return Err(Error::UnverifiedProcess(state.pid));
        }
        #[cfg(target_os = "linux")]
        let process = {
            let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, state.pid, 0) };
            if fd < 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() == Some(libc::ESRCH) {
                    return Err(Error::NotRunning);
                }
                return Err(error).ctx("opening process handle");
            }
            Self {
                fd: unsafe { File::from_raw_fd(fd as i32) },
            }
        };
        #[cfg(not(target_os = "linux"))]
        let process = Self {
            state: state.clone(),
        };
        if !matches(state) {
            return Err(Error::NotRunning);
        }
        Ok(process)
    }

    pub(crate) fn signal(&self, sig: i32) -> Result<bool> {
        #[cfg(target_os = "linux")]
        // PIDFD_SIGNAL_PROCESS_GROUP keeps the signal tied to the verified group.
        let result = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                self.fd.as_raw_fd(),
                sig,
                std::ptr::null::<libc::siginfo_t>(),
                libc::PIDFD_SIGNAL_PROCESS_GROUP,
            )
        };
        #[cfg(not(target_os = "linux"))]
        let result = {
            if identity(self.state.pid).is_some() && !matches(&self.state) {
                return Ok(false);
            }
            unsafe { libc::kill(-self.state.pid, sig) as libc::c_long }
        };
        if result == 0 {
            return Ok(true);
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            return Ok(false);
        }
        #[cfg(target_os = "linux")]
        if error.raw_os_error() == Some(libc::EINVAL) {
            return Err(Error::Config("safe process-group signaling requires Linux 6.9 or later".into()));
        }
        Err(error).ctx("signaling service process")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use gm_core::{GenerationId,
                  RunSource};
    use std::process::Command;

    #[test]
    fn stale_or_missing_identity_never_signals_an_unrelated_process() {
        let mut child = Command::new("sleep").arg("30").spawn().unwrap();
        let mut state = RunState {
            source: RunSource::Generation {
                id: GenerationId(1),
            },
            pid: child.id() as i32,
            process_start: Some("stale identity".into()),
            started_at: Utc::now(),
            detached: true,
        };
        let stale = Process::open(&state);
        state.process_start = None;
        let legacy = Process::open(&state);
        let survived = child.try_wait().unwrap().is_none();
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(matches!(stale, Err(Error::NotRunning)));
        assert!(matches!(legacy, Err(Error::UnverifiedProcess(_))));
        assert!(survived);
    }

    #[test]
    fn nonpositive_pids_have_no_process_identity() {
        assert_eq!(identity(0), None);
        assert_eq!(identity(-1), None);
        assert!(!alive(0));
        assert!(!alive(-1));
    }
}
