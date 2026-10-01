use crate::Journal;
use anyhow::{Context, Result, ensure};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;

/// Snapshot-aware store: external changes are never silently overwritten.
pub struct JournalStore {
    pub path: PathBuf,
    baseline: Option<Vec<u8>>,
    loaded: bool,
}
impl JournalStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            baseline: None,
            loaded: false,
        }
    }
    pub fn default_path() -> Result<PathBuf> {
        directories::ProjectDirs::from("app", "Stillnote", "Stillnote")
            .map(|d| d.data_local_dir().join("journal.json"))
            .context("사용자 데이터 경로를 찾을 수 없습니다")
    }
    pub fn backup_path(&self) -> PathBuf {
        self.path.with_extension("json.bak")
    }
    pub fn load(&mut self) -> Result<Journal> {
        self.loaded = false;
        match fs::read(&self.path) {
            | Ok(bytes) => {
                let journal: Journal = serde_json::from_slice(&bytes).context("저널 파일을 읽을 수 없습니다. 원본을 보존했습니다")?;
                journal.validate().context("저널 데이터 검증 실패. 원본을 보존했습니다")?;
                self.baseline = Some(bytes);
                self.loaded = true;
                Ok(journal)
            },
            | Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.baseline = None;
                self.loaded = true;
                Ok(Journal::default())
            },
            | Err(e) => Err(e).context("저널 파일 접근 실패. 원본을 보존했습니다"),
        }
    }
    pub fn save(&mut self, journal: &Journal) -> Result<()> {
        ensure!(self.loaded, "손상된 파일 보호: 정상 로드 전에는 저장할 수 없습니다");
        journal.validate()?;
        let current = match fs::read(&self.path) {
            | Ok(bytes) => Some(bytes),
            | Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            | Err(e) => return Err(e).context("기존 파일 확인 실패"),
        };
        ensure!(current == self.baseline, "다른 프로그램이 파일을 변경했습니다. 앱을 재시작해 주세요");
        let parent = self.path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
        fs::create_dir_all(parent).context("데이터 폴더 생성 실패")?;
        let bytes = serde_json::to_vec_pretty(journal)?;
        let temp = parent.join(format!(".stillnote-{}.tmp", Uuid::new_v4()));
        let result = (|| -> Result<()> {
            let mut file = OpenOptions::new().write(true).create_new(true).open(&temp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            // Copy backup before replacing. A failed backup aborts without changing the original.
            if self.path.exists() {
                fs::copy(&self.path, self.backup_path()).context("백업 저장 실패")?;
            }
            fs::rename(&temp, &self.path).context("저널 파일 교체 실패")?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(temp);
        }
        result?;
        self.baseline = Some(bytes);
        Ok(())
    }
}
pub struct Session {
    pub journal: Journal,
    pub store: JournalStore,
}
impl Session {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let mut store = JournalStore::new(path);
        let journal = store.load()?;
        Ok(Self { journal, store })
    }
    /// Commit memory only after the durable save succeeds.
    pub fn transact<T>(&mut self, operation: impl FnOnce(&mut Journal) -> Result<T>) -> Result<T> {
        let mut next = self.journal.clone();
        let value = operation(&mut next)?;
        self.store.save(&next)?;
        self.journal = next;
        Ok(value)
    }
}
