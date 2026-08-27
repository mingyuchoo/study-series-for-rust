use crate::{layout::Layout,
            lock::ProjectLock};
use chrono::Utc;
use gm_core::{error::{Error,
                      IoContext,
                      Result},
              generation::{Generation,
                           GenerationId,
                           GenerationStatus,
                           SwitchEvent}};
use std::{fs,
          os::unix::fs::symlink,
          path::{Path,
                 PathBuf}};

/// A generation as found on disk.
#[derive(Debug, Clone)]
pub struct GenerationEntry {
    pub meta: Generation,
    /// The store directory (`.gm/store/0007-a1b2c3d`).
    pub dir: PathBuf,
}

impl GenerationEntry {
    /// Directory the service actually runs from.
    pub fn payload(&self) -> PathBuf { Layout::generation_payload(&self.dir) }
}

#[derive(Debug, Clone)]
pub struct Store {
    layout: Layout,
}

impl Store {
    /// Open the store, creating the state skeleton if this is the first run.
    pub fn open(layout: Layout) -> Result<Store> {
        for dir in [layout.state(), layout.store(), layout.generations(), layout.worktrees(), layout.run_dir()] {
            fs::create_dir_all(&dir).ctx(format!("creating {}", dir.display()))?;
        }
        Ok(Store {
            layout,
        })
    }

    pub fn layout(&self) -> &Layout { &self.layout }

    pub fn lock(&self) -> Result<ProjectLock> { ProjectLock::acquire(&self.layout.lock_file()) }

    // ---------------------------------------------------------------- reading

    /// All generations, ordered oldest first. Unreadable entries are skipped so
    /// one corrupt directory cannot break `gm generations`.
    pub fn list(&self) -> Result<Vec<GenerationEntry>> {
        let generations = self.layout.generations();
        let mut ids = Vec::new();
        for entry in fs::read_dir(&generations).ctx(format!("reading {}", generations.display()))? {
            let entry = entry.ctx("reading generation link")?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let Ok(number) = name.parse::<u64>() else {
                continue;
            };
            ids.push(GenerationId(number));
        }
        ids.sort();

        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            if let Ok(entry) = self.get(id) {
                out.push(entry);
            }
        }
        Ok(out)
    }

    pub fn get(&self, id: GenerationId) -> Result<GenerationEntry> {
        let dir = self.resolve(id)?;
        let meta_path = Layout::generation_meta(&dir);
        let text = fs::read_to_string(&meta_path).map_err(|_| Error::NoSuchGeneration(id.0))?;
        let meta: Generation = serde_json::from_str(&text)?;
        Ok(GenerationEntry {
            meta,
            dir,
        })
    }

    /// Follow `generations/NNNN` to its store directory.
    fn resolve(&self, id: GenerationId) -> Result<PathBuf> {
        let link = self.layout.generation_link(id);
        let target = fs::read_link(&link).map_err(|_| Error::NoSuchGeneration(id.0))?;
        let dir = if target.is_absolute() {
            target
        } else {
            link.parent().unwrap_or(Path::new(".")).join(target)
        };
        dir.canonicalize().map_err(|_| Error::NoSuchGeneration(id.0))
    }

    /// The active generation, or `None` before the first switch.
    pub fn current_id(&self) -> Result<Option<GenerationId>> {
        let link = self.layout.current_link();
        let target = match fs::read_link(&link) {
            | Ok(t) => t,
            | Err(_) => return Ok(None),
        };
        let number = target.file_name().and_then(|n| n.to_str()).and_then(|n| n.parse::<u64>().ok());
        Ok(number.map(GenerationId))
    }

    pub fn current(&self) -> Result<GenerationEntry> {
        let id = self.current_id()?.ok_or(Error::NoCurrentGeneration)?;
        self.get(id)
    }

    /// The generation a bare `gm rollback` would select: the highest id below
    /// the active one, mirroring `nixos-rebuild --rollback`.
    pub fn rollback_target(&self) -> Result<GenerationId> {
        let current = self.current_id()?.ok_or(Error::NoCurrentGeneration)?;
        self.list()?
            .into_iter()
            .map(|e| e.meta.id)
            .filter(|id| *id < current)
            .max()
            .ok_or(Error::NoPreviousGeneration)
    }

    // ---------------------------------------------------------------- writing

    fn next_id(&self) -> Result<GenerationId> {
        let highest = self.list()?.into_iter().map(|e| e.meta.id).max();
        Ok(highest.map(|id| id.next()).unwrap_or(GenerationId(1)))
    }

    /// Create the store directory for the next generation and hand back the
    /// payload directory to populate. Nothing is visible as a generation until
    /// [`Store::commit`] runs, so a failed build leaves no numbered entry.
    pub fn stage(&self, commit: Option<&str>) -> Result<StagedGeneration> {
        let id = self.next_id()?;
        let short: String = commit.unwrap_or("nocommit").chars().take(7).collect();
        let dir = self.layout.store().join(format!("{}-{}", id.dir_prefix(), short));
        if dir.exists() {
            fs::remove_dir_all(&dir).ctx(format!("clearing {}", dir.display()))?;
        }
        let payload = Layout::generation_payload(&dir);
        fs::create_dir_all(&payload).ctx(format!("creating {}", payload.display()))?;
        Ok(StagedGeneration {
            id,
            dir,
            payload,
        })
    }

    /// Write the metadata and publish `generations/NNNN`.
    pub fn commit(&self, staged: StagedGeneration, meta: Generation) -> Result<GenerationEntry> {
        let meta_path = Layout::generation_meta(&staged.dir);
        let text = serde_json::to_string_pretty(&meta)?;
        fs::write(&meta_path, text).ctx(format!("writing {}", meta_path.display()))?;

        let link = self.layout.generation_link(staged.id);
        let _ = fs::remove_file(&link);
        let relative = Path::new("..").join("store").join(staged.dir.file_name().unwrap_or_default());
        symlink(&relative, &link).ctx(format!("linking {}", link.display()))?;

        Ok(GenerationEntry {
            meta,
            dir: staged.dir,
        })
    }

    /// Discard a staged generation that never made it to `commit`.
    pub fn discard(&self, staged: StagedGeneration) { let _ = fs::remove_dir_all(&staged.dir); }

    pub fn set_status(&self, id: GenerationId, status: GenerationStatus) -> Result<()> {
        let entry = self.get(id)?;
        let mut meta = entry.meta;
        meta.status = status;
        let meta_path = Layout::generation_meta(&entry.dir);
        let text = serde_json::to_string_pretty(&meta)?;
        fs::write(&meta_path, text).ctx(format!("writing {}", meta_path.display()))
    }

    /// Repoint `current` at `id`. The temp-symlink-then-rename dance is what
    /// makes this atomic: `ln -sfn` unlinks first and leaves a window where the
    /// project has no active generation.
    pub fn switch(&self, id: GenerationId, reason: &str) -> Result<()> {
        let previous = self.current_id()?;
        // Fail before touching anything if the target is not a real generation.
        self.get(id)?;

        let current = self.layout.current_link();
        let temp = self.layout.state().join(format!(".current.tmp.{}", std::process::id()));
        let _ = fs::remove_file(&temp);

        let target = Path::new("generations").join(id.dir_prefix());
        symlink(&target, &temp).ctx(format!("creating {}", temp.display()))?;
        fs::rename(&temp, &current)
            .inspect_err(|_| {
                let _ = fs::remove_file(&temp);
            })
            .ctx(format!("activating generation {id}"))?;

        self.append_history(&SwitchEvent {
            at: Utc::now(),
            from: previous,
            to: id,
            reason: reason.to_string(),
        })
    }

    fn append_history(&self, event: &SwitchEvent) -> Result<()> {
        use std::io::Write;
        let path = self.layout.history();
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .ctx(format!("opening {}", path.display()))?;
        let line = serde_json::to_string(event)?;
        writeln!(file, "{line}").ctx(format!("writing {}", path.display()))
    }

    pub fn history(&self) -> Result<Vec<SwitchEvent>> {
        let path = self.layout.history();
        let Ok(text) = fs::read_to_string(&path) else {
            return Ok(Vec::new());
        };
        Ok(text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect())
    }

    /// Delete all but the `keep` most recent generations. The active generation
    /// and its rollback target are always retained.
    pub fn gc(&self, keep: usize) -> Result<Vec<GenerationId>> {
        let keep = keep.max(1);
        let all = self.list()?;
        if all.len() <= keep {
            return Ok(Vec::new());
        }
        let current = self.current_id()?;
        let rollback = self.rollback_target().ok();
        let doomed: Vec<_> = all[.. all.len() - keep]
            .iter()
            .map(|e| e.meta.id)
            .filter(|id| Some(*id) != current && Some(*id) != rollback)
            .collect();

        let mut removed = Vec::new();
        for id in doomed {
            let dir = self.resolve(id)?;
            fs::remove_dir_all(&dir).ctx(format!("removing {}", dir.display()))?;
            let link = self.layout.generation_link(id);
            fs::remove_file(&link).ctx(format!("removing {}", link.display()))?;
            removed.push(id);
        }
        Ok(removed)
    }
}

/// A generation directory that exists on disk but is not yet numbered.
#[derive(Debug)]
pub struct StagedGeneration {
    pub id: GenerationId,
    pub dir: PathBuf,
    pub payload: PathBuf,
}
