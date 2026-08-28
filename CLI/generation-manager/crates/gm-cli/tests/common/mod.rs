//! Shared fixtures for the CLI integration tests.
//!
//! Every test drives the real `gm` binary against a throwaway project in the
//! temp directory, so what is exercised is the tool as a user meets it.

#![allow(dead_code)]

use gm_core::run::RunState;
use std::{path::{Path,
                 PathBuf},
          process::{Command,
                    Output},
          sync::atomic::{AtomicU32,
                         Ordering}};

pub const GM: &str = env!("CARGO_BIN_EXE_gm");

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// Builder for `generation-manager.toml`.
///
/// The defaults describe the smallest project that still exercises the whole
/// pipeline: a build stage that produces one artifact, and a health check that
/// passes.
pub struct Manifest {
    build: String,
    test: Option<String>,
    artifacts: Vec<String>,
    run: String,
    health: Option<String>,
    health_timeout: u64,
}

impl Manifest {
    pub fn new(run: &str) -> Manifest {
        Manifest {
            build: "echo built > built.marker".to_string(),
            test: None,
            artifacts: vec!["built.marker".to_string()],
            run: run.to_string(),
            health: Some("true".to_string()),
            health_timeout: 2,
        }
    }

    pub fn build(mut self, cmd: &str) -> Manifest {
        self.build = cmd.to_string();
        self
    }

    pub fn test(mut self, cmd: &str) -> Manifest {
        self.test = Some(cmd.to_string());
        self
    }

    pub fn artifacts(mut self, paths: &[&str]) -> Manifest {
        self.artifacts = paths.iter().map(|p| p.to_string()).collect();
        self
    }

    /// Shell command run from the generation payload; exit 0 means healthy.
    pub fn health(mut self, cmd: &str) -> Manifest {
        self.health = Some(cmd.to_string());
        self
    }

    pub fn no_health(mut self) -> Manifest {
        self.health = None;
        self
    }

    fn render(&self) -> String {
        let artifacts: Vec<String> = self.artifacts.iter().map(|p| format!("\"{p}\"")).collect();
        let mut text = format!(
            "[project]\nname = \"demo\"\n\n\
             [build]\ncmd = \"{}\"\n\n\
             [artifacts]\ninclude = [{}]\n\n\
             [run]\ncmd = \"{}\"\nstop_timeout_secs = 2\n",
            self.build,
            artifacts.join(", "),
            self.run,
        );
        if let Some(test) = &self.test {
            text.push_str(&format!("\n[test]\ncmd = \"{test}\"\n"));
        }
        if let Some(health) = &self.health {
            text.push_str(&format!(
                "\n[health]\ncmd = \"{health}\"\ntimeout_secs = {}\ninterval_secs = 1\n",
                self.health_timeout
            ));
        }
        text
    }
}

pub struct Fixture {
    pub root: PathBuf,
}

impl Fixture {
    /// A directory with no manifest, for testing `gm project init` and
    /// discovery failure.
    pub fn bare() -> Fixture {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!("gm-it-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        Fixture {
            root,
        }
    }

    pub fn new(run_cmd: &str) -> Fixture { Fixture::with(Manifest::new(run_cmd)) }

    pub fn with(manifest: Manifest) -> Fixture {
        let fixture = Fixture::bare();
        fixture.set_manifest(&manifest);
        fixture
    }

    pub fn set_manifest(&self, manifest: &Manifest) { std::fs::write(self.root.join("generation-manager.toml"), manifest.render()).unwrap(); }

    // ------------------------------------------------------------ filesystem

    pub fn path(&self, relative: &str) -> PathBuf { self.root.join(relative) }

    pub fn write(&self, relative: &str, contents: &str) {
        let path = self.path(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }

    pub fn read(&self, relative: &str) -> String { std::fs::read_to_string(self.path(relative)).unwrap() }

    pub fn worktree_dir(&self, name: &str) -> PathBuf { self.root.join(".gm/worktrees").join(name) }

    /// A worktree directory without git behind it. Enough for the commands that
    /// only need the checkout to exist.
    pub fn fake_worktree(&self, name: &str) -> PathBuf {
        let path = self.worktree_dir(name);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    /// The payload directory of a generation, as `gm` laid it out.
    pub fn generation_payload(&self, id: u64) -> PathBuf { self.root.join(format!(".gm/generations/{id:04}/root")) }

    pub fn run_state(&self) -> Option<RunState> {
        let text = std::fs::read_to_string(self.root.join(".gm/run/state.json")).ok()?;
        serde_json::from_str(&text).ok()
    }

    // ------------------------------------------------------------------- git

    pub fn git(&self, dir: &Path, args: &[&str]) -> Output { Command::new("git").args(args).current_dir(dir).output().expect("failed to run git") }

    /// Initialise a repository with one commit, which `git worktree add` needs.
    pub fn git_init(&self) {
        self.write(".gitignore", ".gm/\nbuilt.marker\npayload.txt\n");
        self.git(&self.root, &["init", "-q", "-b", "main"]);
        self.git(&self.root, &["config", "user.email", "test@example.com"]);
        self.git(&self.root, &["config", "user.name", "test"]);
        self.git(&self.root, &["add", "-A"]);
        self.git(&self.root, &["commit", "-qm", "initial"]);
    }

    // -------------------------------------------------------------------- gm

    pub fn gm(&self, args: &[&str]) -> Output { self.gm_in(&self.root, args) }

    pub fn gm_in(&self, cwd: &Path, args: &[&str]) -> Output { Command::new(GM).args(args).current_dir(cwd).output().expect("failed to run the gm binary") }

    /// Run `gm` and fail the test if it did not succeed.
    pub fn gm_ok(&self, args: &[&str]) -> Output {
        let output = self.gm(args);
        assert!(
            output.status.success(),
            "`gm {}` failed\nstdout: {}\nstderr: {}",
            args.join(" "),
            stdout(&output),
            stderr(&output)
        );
        output
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Tests that leave a detached process must not outlive their directory.
        let _ = self.gm(&["service", "stop"]);
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub fn stdout(output: &Output) -> String { String::from_utf8_lossy(&output.stdout).into_owned() }

pub fn stderr(output: &Output) -> String { String::from_utf8_lossy(&output.stderr).into_owned() }

/// Both streams, for assertions that do not care which one carried the message.
pub fn combined(output: &Output) -> String { format!("{}{}", stdout(output), stderr(output)) }
