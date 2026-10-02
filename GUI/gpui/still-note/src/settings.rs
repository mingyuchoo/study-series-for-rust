use anyhow::{Context, Result, ensure};
use gpui::WindowAppearance;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    #[default]
    Korean,
    English,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}
impl ThemeMode {
    pub fn resolve(self, appearance: WindowAppearance) -> Self {
        match self {
            | Self::System => match appearance {
                | WindowAppearance::Light | WindowAppearance::VibrantLight => Self::Light,
                | WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::Dark,
            },
            | fixed => fixed,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub language: Language,
    pub theme: ThemeMode,
}

/// Keep invalid or externally changed settings intact, independently of the journal.
pub struct SettingsStore {
    pub path: PathBuf,
    baseline: Option<Vec<u8>>,
    loaded: bool,
}
impl SettingsStore {
    pub fn new(journal_path: impl AsRef<Path>) -> Self {
        Self {
            path: journal_path.as_ref().with_file_name("settings.json"),
            baseline: None,
            loaded: false,
        }
    }
    pub fn load(&mut self) -> Result<Settings> {
        self.loaded = false;
        match fs::read(&self.path) {
            | Ok(bytes) => {
                let settings = serde_json::from_slice(&bytes).context(crate::i18n::Message::new("설정 파일을 읽을 수 없습니다. 원본을 보존했습니다"))?;
                self.baseline = Some(bytes);
                self.loaded = true;
                Ok(settings)
            },
            | Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.baseline = None;
                self.loaded = true;
                Ok(Settings::default())
            },
            | Err(e) => Err(e).context(crate::i18n::Message::new("설정 파일 접근 실패. 원본을 보존했습니다")),
        }
    }
    pub fn save(&mut self, settings: &Settings) -> Result<()> {
        ensure!(self.loaded, crate::i18n::Message::new("설정 원본 보호: 파일을 확인한 뒤 앱을 재시작해 주세요"));
        let current = match fs::read(&self.path) {
            | Ok(bytes) => Some(bytes),
            | Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            | Err(e) => return Err(e).context(crate::i18n::Message::new("설정 파일 접근 실패. 원본을 보존했습니다")),
        };
        ensure!(
            current == self.baseline,
            crate::i18n::Message::new("다른 프로그램이 설정을 변경했습니다. 앱을 재시작해 주세요")
        );
        let parent = self.path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
        fs::create_dir_all(parent)?;
        let bytes = serde_json::to_vec_pretty(settings)?;
        let temp = parent.join(format!(".stillnote-settings-{}.tmp", Uuid::new_v4()));
        let result = (|| -> Result<()> {
            let mut file = OpenOptions::new().write(true).create_new(true).open(&temp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temp, &self.path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result.context(crate::i18n::Message::new("설정을 저장하지 못했습니다. 선택은 현재 세션에 적용됩니다"))?;
        self.baseline = Some(bytes);
        Ok(())
    }
}
