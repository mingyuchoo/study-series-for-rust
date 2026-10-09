use crate::{error::{Error,
                    IoContext,
                    Result},
            process::{self,
                      Process}};
use chrono::Utc;
use gm_application::{DevelopmentRuntime,
                     ForegroundProcess,
                     RunStateRepository};
use gm_core::{config::RunStage,
              run::{RunSource,
                    RunState}};
use std::{fs::{File,
               OpenOptions},
          os::unix::process::{CommandExt,
                              ExitStatusExt},
          path::{Path,
                 PathBuf},
          process::{Child,
                    Command,
                    Stdio},
          sync::Arc,
          time::{Duration,
                 Instant}};

/// Minimal supervisor for a single local process.
///
/// One project owns one service slot. Whatever takes the slot — an activated
/// generation or a development worktree — records itself in `run/state.json`,
/// so `gm project status` can always name what is running, not merely that
/// something is.
///
/// The scope is deliberate: one process, one state file, one log file. A
/// multi-service or reboot-surviving setup belongs to systemd, and this type is
/// the seam where such a backend would slot in.
#[derive(Clone)]
pub struct Supervisor {
    state: Arc<dyn RunStateRepository>,
    log_file: PathBuf,
}

impl gm_application::ServiceRuntime for Supervisor {
    fn stop_if_running(&self, timeout: Duration) -> gm_application::PortResult<Option<RunState>> { Ok(Supervisor::stop_if_running(self, timeout)?) }

    fn start_detached(&self, run: &RunStage, cwd: &Path, source: RunSource) -> gm_application::PortResult<RunState> {
        Ok(Supervisor::start_detached(self, run, cwd, source)?)
    }
}

#[derive(Debug, Clone)]
pub enum ServiceStatus {
    Running(RunState),
    Stopped,
}

impl Supervisor {
    pub fn new(state: impl RunStateRepository + 'static, log_file: PathBuf) -> Supervisor {
        Supervisor {
            state: Arc::new(state),
            log_file,
        }
    }

    pub fn log_file(&self) -> &Path { &self.log_file }

    /// A state file whose pid is no longer alive reads as stopped, so a crashed
    /// service or an interrupted foreground run heals itself.
    pub fn status(&self) -> ServiceStatus {
        match self.read_state() {
            | Some(state) if process::alive(state.pid) && (state.process_start.is_none() || process::matches(&state)) => ServiceStatus::Running(state),
            | _ => ServiceStatus::Stopped,
        }
    }

    pub fn running(&self) -> Option<RunState> {
        match self.status() {
            | ServiceStatus::Running(state) => Some(state),
            | ServiceStatus::Stopped => None,
        }
    }

    fn read_state(&self) -> Option<RunState> { self.state.read() }

    fn write_state(&self, state: &RunState) -> Result<()> { self.state.write(state).map_err(Error::Repository) }

    fn record_child(&self, child: &mut Child, state: &RunState) -> Result<()> {
        let recorded = Process::open(state).and_then(|process| process.signal(0)).and_then(|_| self.write_state(state));
        if let Err(error) = recorded {
            process::terminate(child).ctx(format!("cleaning up child after recording failed: {error}"))?;
            return Err(error);
        }
        Ok(())
    }

    fn clear_state(&self) { self.state.clear(); }

    fn ensure_slot_free(&self) -> Result<()> {
        match self.status() {
            | ServiceStatus::Running(state) => Err(Error::AlreadyRunning(state.pid)),
            | ServiceStatus::Stopped => Ok(()),
        }
    }

    /// Launch in the background, detached into its own session so the process
    /// outlives the `gm` invocation, with output appended to the log file.
    pub fn start_detached(&self, run: &RunStage, cwd: &Path, source: RunSource) -> Result<RunState> {
        self.ensure_slot_free()?;

        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file)
            .ctx(format!("opening {}", self.log_file.display()))?;
        let log_err = log.try_clone().ctx(format!("cloning {}", self.log_file.display()))?;

        let mut command = self.base_command(run, cwd);
        command.stdin(Stdio::null()).stdout(Stdio::from(log)).stderr(Stdio::from(log_err));

        // Detach from the terminal's process group so Ctrl-C on `gm` does not
        // take the service down with it.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }

        let mut child = command.spawn().ctx(format!("spawning `{}`", run.cmd))?;
        let state = RunState {
            source,
            pid: child.id() as i32,
            process_start: process::identity(child.id() as i32),
            started_at: Utc::now(),
            detached: true,
        };
        self.record_child(&mut child, &state)?;
        Ok(state)
    }

    /// Start attached to the terminal. Used by `gm worktree run`: in the inner
    /// development loop you want the output in front of you and Ctrl-C to mean
    /// stop.
    ///
    /// The child owns a separate process group and, when attached to a TTY,
    /// takes the terminal so Ctrl-C reaches the complete run.
    ///
    /// Spawning and waiting are separate so the caller can release the project
    /// lock once the slot is taken, instead of holding it for the whole
    /// session.
    pub fn spawn_foreground(&self, run: &RunStage, cwd: &Path, source: RunSource) -> Result<ForegroundRun> {
        self.ensure_slot_free()?;

        let mut command = self.base_command(run, cwd);
        command.process_group(0);
        command.stdin(Stdio::inherit()).stdout(Stdio::inherit()).stderr(Stdio::inherit());

        let mut child = command.spawn().ctx(format!("spawning `{}`", run.cmd))?;
        let state = RunState {
            source,
            pid: child.id() as i32,
            process_start: process::identity(child.id() as i32),
            started_at: Utc::now(),
            detached: false,
        };
        self.record_child(&mut child, &state)?;
        let terminal_group = match terminal_group(child.id() as i32) {
            | Ok(group) => group,
            | Err(error) => {
                process::terminate(&mut child).ctx("cleaning up after terminal handoff failed")?;
                self.clear_state();
                return Err(error);
            },
        };

        Ok(ForegroundRun {
            child,
            supervisor: self.clone(),
            terminal_group,
            state,
        })
    }

    fn base_command(&self, run: &RunStage, cwd: &Path) -> Command {
        let mut command = Command::new("sh");
        command.arg("-c").arg(&run.cmd).current_dir(cwd).envs(&run.env);
        command
    }

    /// SIGTERM, wait up to `timeout`, then SIGKILL. Returns what was stopped so
    /// the caller can report displacing someone else's process.
    pub fn stop(&self, timeout: Duration) -> Result<RunState> {
        let ServiceStatus::Running(state) = self.status() else {
            self.clear_state();
            return Err(Error::NotRunning);
        };

        let process = Process::open(&state)?;
        process.signal(libc::SIGTERM)?;
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if !process.signal(0)? {
                self.clear_state();
                return Ok(state);
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        process.signal(libc::SIGKILL)?;
        std::thread::sleep(Duration::from_millis(200));
        self.clear_state();
        Ok(state)
    }

    /// Stop if running; `Ok(None)` when the slot was already free.
    pub fn stop_if_running(&self, timeout: Duration) -> Result<Option<RunState>> {
        match self.stop(timeout) {
            | Ok(state) => Ok(Some(state)),
            | Err(Error::NotRunning) => Ok(None),
            | Err(other) => Err(other),
        }
    }

    /// Last `lines` lines of the service log.
    pub fn tail(&self, lines: usize) -> Result<String> {
        let Ok(text) = std::fs::read_to_string(&self.log_file) else {
            return Ok(String::new());
        };
        let collected: Vec<&str> = text.lines().collect();
        let start = collected.len().saturating_sub(lines);
        Ok(collected[start ..].join("\n"))
    }

    pub fn truncate_log(&self) -> Result<()> {
        File::create(&self.log_file).ctx(format!("truncating {}", self.log_file.display()))?;
        Ok(())
    }
}

/// A foreground service that has taken the slot but not yet exited.
pub struct ForegroundRun {
    child: Child,
    supervisor: Supervisor,
    terminal_group: Option<i32>,
    state: RunState,
}

impl ForegroundRun {
    pub fn pid(&self) -> i32 { self.child.id() as i32 }

    /// Block until the process exits, then free the slot.
    pub fn wait(mut self) -> Result<i32> {
        let status = self.child.wait().ctx("waiting for the service")?;
        if let Some(group) = self.terminal_group {
            set_terminal_group(group)?;
        }
        self.supervisor.state.clear_if_matches(&self.state).map_err(Error::Repository)?;
        Ok(status.code().unwrap_or_else(|| 128 + status.signal().unwrap_or(0)))
    }
}

fn terminal_group(pid: i32) -> Result<Option<i32>> {
    let previous = unsafe { libc::tcgetpgrp(libc::STDIN_FILENO) };
    if previous == -1 {
        return Ok(None);
    }
    set_terminal_group(pid)?;
    if unsafe { libc::kill(-pid, libc::SIGCONT) } == -1 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::ESRCH) {
            return Err(error).ctx("resuming foreground service");
        }
    }
    Ok(Some(previous))
}

fn set_terminal_group(group: i32) -> Result<()> {
    unsafe {
        let mut mask: libc::sigset_t = std::mem::zeroed();
        let mut old: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut mask);
        libc::sigaddset(&mut mask, libc::SIGTTOU);
        let blocked = libc::pthread_sigmask(libc::SIG_BLOCK, &mask, &mut old);
        if blocked != 0 {
            return Err(std::io::Error::from_raw_os_error(blocked)).ctx("blocking terminal stop signal");
        }
        let result = libc::tcsetpgrp(libc::STDIN_FILENO, group);
        let error = std::io::Error::last_os_error();
        let restored = libc::pthread_sigmask(libc::SIG_SETMASK, &old, std::ptr::null_mut());
        if result == -1 {
            return Err(error).ctx("changing foreground terminal group");
        }
        if restored != 0 {
            return Err(std::io::Error::from_raw_os_error(restored)).ctx("restoring terminal signal mask");
        }
    }
    Ok(())
}

impl DevelopmentRuntime for Supervisor {
    type Foreground = ForegroundRun;

    fn spawn_foreground(&self, run: &RunStage, cwd: &Path, source: RunSource) -> gm_application::PortResult<ForegroundRun> {
        Ok(Supervisor::spawn_foreground(self, run, cwd, source)?)
    }
}

impl ForegroundProcess for ForegroundRun {
    fn wait(self) -> gm_application::PortResult<i32> { Ok(ForegroundRun::wait(self)?) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gm_core::GenerationId;
    use std::sync::Mutex;

    #[derive(Clone, Default)]
    struct MemoryState(Arc<Mutex<(Option<RunState>, bool)>>);
    impl RunStateRepository for MemoryState {
        fn read(&self) -> Option<RunState> { self.0.lock().unwrap().0.clone() }

        fn write(&self, state: &RunState) -> gm_application::PortResult<()> {
            let mut slot = self.0.lock().unwrap();
            if slot.1 {
                return Err(std::io::Error::other("injected state write failure").into());
            }
            slot.0 = Some(state.clone());
            Ok(())
        }

        fn clear(&self) { self.0.lock().unwrap().0 = None; }

        fn clear_if_matches(&self, state: &RunState) -> gm_application::PortResult<()> {
            let mut slot = self.0.lock().unwrap();
            if slot
                .0
                .as_ref()
                .is_some_and(|current| current.pid == state.pid && current.process_start == state.process_start && current.started_at == state.started_at)
            {
                slot.0 = None;
            }
            Ok(())
        }
    }
    use std::sync::atomic::{AtomicU32,
                            Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);
    struct Fixture {
        root: PathBuf,
        state: MemoryState,
        supervisor: Supervisor,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("gm-supervisor-{}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed)));
            std::fs::create_dir_all(&root).unwrap();
            let state = MemoryState::default();
            let supervisor = Supervisor::new(state.clone(), root.join("service.log"));
            Self {
                root,
                state,
                supervisor,
            }
        }

        fn run(&self, cmd: &str) -> RunStage {
            RunStage {
                cmd: cmd.into(),
                env: Default::default(),
                stop_timeout_secs: 0,
            }
        }

        fn source(&self) -> RunSource {
            RunSource::Generation {
                id: GenerationId(1),
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = self.supervisor.stop_if_running(Duration::ZERO);
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn stopping_a_shell_also_stops_its_children() {
        for command in [
            "sleep 30 & echo $! > child.pid; wait",
            "sh -c 'trap \"\" TERM; echo $$ > child.pid; exec sleep 30' & wait",
        ] {
            let fixture = Fixture::new();
            fixture
                .supervisor
                .start_detached(&fixture.run(command), &fixture.root, fixture.source())
                .unwrap();
            let marker = fixture.root.join("child.pid");
            let deadline = Instant::now() + Duration::from_secs(2);
            let child: i32 = loop {
                if let Ok(text) = std::fs::read_to_string(&marker)
                    && let Ok(pid) = text.trim().parse()
                {
                    break pid;
                }
                assert!(Instant::now() < deadline, "child did not start");
                std::thread::sleep(Duration::from_millis(10));
            };
            fixture.supervisor.stop(Duration::from_millis(100)).unwrap();
            assert!(!process::alive(child));
        }
    }

    #[test]
    fn an_old_foreground_waiter_does_not_clear_a_replacement_run() {
        let fixture = Fixture::new();
        let first = {
            fixture
                .supervisor
                .spawn_foreground(&fixture.run("exec sleep 30"), &fixture.root, fixture.source())
                .unwrap()
        };
        let second = {
            fixture.supervisor.stop(Duration::ZERO).unwrap();
            fixture
                .supervisor
                .start_detached(&fixture.run("exec sleep 30"), &fixture.root, fixture.source())
                .unwrap()
        };
        first.wait().unwrap();
        let running = fixture.supervisor.running();
        // Keep cleanup independent of the shared file, which the regression can
        // erase.
        Process::open(&second).unwrap().signal(libc::SIGKILL).unwrap();
        assert_eq!(running.unwrap().pid, second.pid);
    }

    #[test]
    fn a_failed_state_write_terminates_and_reaps_the_child() {
        let fixture = Fixture::new();
        fixture.state.0.lock().unwrap().1 = true;
        let mut child = Command::new("sh").args(["-c", "exec sleep 30"]).process_group(0).spawn().unwrap();
        let state = RunState {
            source: fixture.source(),
            pid: child.id() as i32,
            process_start: process::identity(child.id() as i32),
            started_at: Utc::now(),
            detached: true,
        };
        assert!(fixture.supervisor.record_child(&mut child, &state).is_err());
        assert!(child.try_wait().unwrap().is_some());
        assert!(!process::alive(state.pid));
    }
}
