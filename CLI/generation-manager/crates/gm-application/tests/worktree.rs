use gm_application::{DevelopmentRuntime,
                     ForegroundProcess,
                     PortResult,
                     RunWorktree,
                     ServiceRuntime,
                     StageExecutor,
                     WorktreeControl,
                     WorktreeLocation,
                     WorktreeRepository,
                     WorktreeRun,
                     create_worktree,
                     remove_worktree,
                     run_worktree};
use gm_core::{Preset,
              RunSource,
              RunStage,
              RunState};
use std::{cell::RefCell,
          collections::BTreeMap,
          path::{Path,
                 PathBuf},
          time::Duration};

#[derive(Default)]
struct Effects {
    calls: RefCell<Vec<String>>,
    fail: Option<&'static str>,
    exists: bool,
}

impl Effects {
    fn call(&self, name: &str) -> PortResult<()> {
        self.calls.borrow_mut().push(name.into());
        if self.fail == Some(name) {
            Err(std::io::Error::other(name).into())
        } else {
            Ok(())
        }
    }
}

impl StageExecutor for Effects {
    fn run(&self, name: &str, _: &str, _: &Path, _: &BTreeMap<String, String>) -> PortResult<()> { self.call(name) }
}

struct Foreground;
impl ForegroundProcess for Foreground {
    fn wait(self) -> PortResult<i32> { Ok(7) }
}

impl ServiceRuntime for Effects {
    fn stop_if_running(&self, _: Duration) -> PortResult<Option<RunState>> {
        self.call("stop")?;
        Ok(None)
    }

    fn start_detached(&self, _: &RunStage, cwd: &Path, source: RunSource) -> PortResult<RunState> {
        assert_eq!(cwd, Path::new("/project/worktrees/feature"));
        assert!(matches!(&source, RunSource::Worktree { name } if name == "feature"));
        self.call("detach")?;
        Ok(RunState {
            source,
            pid: 42,
            process_start: None,
            started_at: chrono::DateTime::UNIX_EPOCH,
            detached: true,
        })
    }
}

impl DevelopmentRuntime for Effects {
    type Foreground = Foreground;

    fn spawn_foreground(&self, _: &RunStage, _: &Path, _: RunSource) -> PortResult<Foreground> {
        self.call("spawn")?;
        Ok(Foreground)
    }
}

impl WorktreeRepository for Effects {
    fn project_root(&self) -> &Path { Path::new("/project") }

    fn locate(&self, name: &str) -> PortResult<WorktreeLocation> {
        Ok(WorktreeLocation {
            path: PathBuf::from("/project/worktrees").join(name),
            exists: self.exists,
        })
    }
}

impl WorktreeControl for Effects {
    fn is_repo(&self, _: &Path) -> bool { true }

    fn add(&self, _: &Path, path: &Path, name: &str, base: Option<&str>) -> PortResult<()> {
        assert_eq!(path, Path::new("/project/worktrees/feature"));
        assert_eq!(name, "feature");
        assert_eq!(base, Some("main"));
        self.call("add")
    }

    fn remove(&self, _: &Path, _: &Path, force: bool) -> PortResult<()> {
        assert!(force);
        self.call("remove")
    }

    fn prune(&self, _: &Path) { self.call("prune").unwrap(); }
}

fn request(build: bool, detach: bool) -> RunWorktree<'static> {
    RunWorktree {
        name: "feature",
        path: Path::new("/project/worktrees/feature"),
        build,
        detach,
    }
}

#[test]
fn development_runs_build_only_and_return_an_unwaited_foreground_handle() {
    let effects = Effects::default();
    let result = run_worktree(request(true, false), &Preset::Generic.template("demo"), &effects, &effects).unwrap();
    let WorktreeRun::Foreground(handle) = result else {
        panic!("expected foreground")
    };
    assert_eq!(*effects.calls.borrow(), ["stop", "build", "spawn"]);
    assert_eq!(handle.wait().unwrap(), 7);
}

#[test]
fn detached_runs_can_skip_build_and_failures_prevent_launching() {
    let config = Preset::Generic.template("demo");
    let effects = Effects::default();
    assert!(matches!(
        run_worktree(request(false, true), &config, &effects, &effects).unwrap(),
        WorktreeRun::Detached(RunState {
            pid: 42,
            ..
        })
    ));
    assert_eq!(*effects.calls.borrow(), ["stop", "detach"]);
    for (failure, expected) in [("stop", vec!["stop"]), ("build", vec!["stop", "build"])] {
        let effects = Effects {
            fail: Some(failure),
            ..Default::default()
        };
        assert!(run_worktree(request(true, false), &config, &effects, &effects).is_err());
        assert_eq!(*effects.calls.borrow(), expected);
    }
}

#[test]
fn worktree_preconditions_prevent_git_mutations_and_failed_remove_skips_prune() {
    let effects = Effects::default();
    create_worktree("feature", Some("main"), &effects, &effects).unwrap();
    assert_eq!(*effects.calls.borrow(), ["add"]);
    assert!(remove_worktree("feature", true, &effects, &effects).is_err());
    assert!(create_worktree("../outside", Some("main"), &effects, &effects).is_err());
    assert_eq!(*effects.calls.borrow(), ["add"]);
    let effects = Effects {
        exists: true,
        ..Default::default()
    };
    assert!(create_worktree("feature", Some("main"), &effects, &effects).is_err());
    remove_worktree("feature", true, &effects, &effects).unwrap();
    assert_eq!(*effects.calls.borrow(), ["remove", "prune"]);
    let effects = Effects {
        exists: true,
        fail: Some("remove"),
        ..Default::default()
    };
    assert!(remove_worktree("feature", true, &effects, &effects).is_err());
    assert_eq!(*effects.calls.borrow(), ["remove"]);
}
