use crate::error::{Error,
                   IoContext,
                   Result};
use chrono::Utc;
use gm_core::{config::RunStage,
              run::{RunSource,
                    RunState}};
use gm_store::Layout;
use std::{fs::{File,
               OpenOptions},
          os::unix::process::CommandExt,
          path::{Path,
                 PathBuf},
          process::{Child,
                    Command,
                    Stdio},
          time::{Duration,
                 Instant}};

/// Minimal supervisor for a single local process.
///
/// One project owns one service slot. Whatever takes the slot — an activated
/// generation or a development worktree — records itself in `run/state.json`,
/// so `gm status` can always name what is running, not merely that something
/// is.
///
/// The scope is deliberate: one process, one state file, one log file. A
/// multi-service or reboot-surviving setup belongs to systemd, and this type is
/// the seam where such a backend would slot in.
#[derive(Debug, Clone)]
pub struct Supervisor {
    state_file: PathBuf,
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
    pub fn new(layout: &Layout) -> Supervisor {
        Supervisor {
            state_file: layout.run_state(),
            log_file: layout.log_file(),
        }
    }

    pub fn log_file(&self) -> &Path { &self.log_file }

    /// A state file whose pid is no longer alive reads as stopped, so a crashed
    /// service or an interrupted foreground run heals itself.
    pub fn status(&self) -> ServiceStatus {
        match self.read_state() {
            | Some(state) if pid_alive(state.pid) => ServiceStatus::Running(state),
            | _ => ServiceStatus::Stopped,
        }
    }

    pub fn running(&self) -> Option<RunState> {
        match self.status() {
            | ServiceStatus::Running(state) => Some(state),
            | ServiceStatus::Stopped => None,
        }
    }

    fn read_state(&self) -> Option<RunState> {
        let text = std::fs::read_to_string(&self.state_file).ok()?;
        serde_json::from_str(&text).ok()
    }

    fn write_state(&self, state: &RunState) -> Result<()> {
        let text = serde_json::to_string_pretty(state)?;
        std::fs::write(&self.state_file, text).ctx(format!("writing {}", self.state_file.display()))
    }

    fn clear_state(&self) { let _ = std::fs::remove_file(&self.state_file); }

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
                libc::setsid();
                Ok(())
            });
        }

        let child = command.spawn().ctx(format!("spawning `{}`", run.cmd))?;
        let state = RunState {
            source,
            pid: child.id() as i32,
            started_at: Utc::now(),
            detached: true,
        };
        self.write_state(&state)?;
        Ok(state)
    }

    /// Start attached to the terminal. Used by `gm dev run`: in the inner
    /// development loop you want the output in front of you and Ctrl-C to mean
    /// stop.
    ///
    /// No `setsid` here on purpose — the child stays in this process group so
    /// the terminal's signals reach it.
    ///
    /// Spawning and waiting are separate so the caller can release the project
    /// lock once the slot is taken, instead of holding it for the whole
    /// session.
    pub fn spawn_foreground(&self, run: &RunStage, cwd: &Path, source: RunSource) -> Result<ForegroundRun> {
        self.ensure_slot_free()?;

        let mut command = self.base_command(run, cwd);
        command.stdin(Stdio::inherit()).stdout(Stdio::inherit()).stderr(Stdio::inherit());

        let child = command.spawn().ctx(format!("spawning `{}`", run.cmd))?;
        let state = RunState {
            source,
            pid: child.id() as i32,
            started_at: Utc::now(),
            detached: false,
        };
        self.write_state(&state)?;

        Ok(ForegroundRun {
            child,
            supervisor: self.clone(),
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

        signal(state.pid, libc::SIGTERM);
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if !pid_alive(state.pid) {
                self.clear_state();
                return Ok(state);
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        signal(state.pid, libc::SIGKILL);
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
#[derive(Debug)]
pub struct ForegroundRun {
    child: Child,
    supervisor: Supervisor,
}

impl ForegroundRun {
    pub fn pid(&self) -> i32 { self.child.id() as i32 }

    /// Block until the process exits, then free the slot.
    pub fn wait(mut self) -> Result<i32> {
        let status = self.child.wait().ctx("waiting for the service")?;
        self.supervisor.clear_state();
        Ok(status.code().unwrap_or(-1))
    }
}

fn pid_alive(pid: i32) -> bool {
    // Signal 0 performs the permission and existence check without delivering.
    unsafe { libc::kill(pid, 0) == 0 }
}

fn signal(pid: i32, sig: i32) {
    unsafe {
        libc::kill(pid, sig);
    }
}
