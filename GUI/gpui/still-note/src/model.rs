use anyhow::{Result, bail, ensure};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Task,
    Event,
    Note,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    Open,
    Complete,
    Cancelled,
    Migrated,
    Scheduled,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Log {
    Daily,
    Monthly,
    Future,
    Collection(Uuid),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub id: Uuid,
    pub date: NaiveDate,
    pub log: Log,
    pub kind: Kind,
    pub status: Status,
    pub important: bool,
    pub text: String,
    pub migrated_from: Option<Uuid>,
    pub migrated_to: Option<Uuid>,
}
impl Entry {
    pub fn symbol(&self) -> &'static str {
        match self.status {
            Status::Complete => "×",
            Status::Cancelled => "⊘",
            Status::Migrated => ">",
            Status::Scheduled => "<",
            Status::Open => match self.kind {
                Kind::Task => "•",
                Kind::Event => "○",
                Kind::Note => "–",
            },
        }
    }
    pub fn is_open_task(&self) -> bool {
        self.kind == Kind::Task && self.status == Status::Open
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collection {
    pub id: Uuid,
    pub name: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Journal {
    pub version: u32,
    pub entries: Vec<Entry>,
    pub collections: Vec<Collection>,
}
impl Default for Journal {
    fn default() -> Self {
        Self {
            version: 1,
            entries: vec![],
            collections: vec![],
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    All,
    Open,
    Complete,
}
pub fn parse_date(value: &str) -> Result<NaiveDate> {
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d")?;
    ensure!(
        (1..=9999).contains(&date.year()),
        "날짜는 0001~9999년 범위여야 합니다"
    );
    Ok(date)
}
pub fn shift_month(date: NaiveDate, delta: i32) -> Result<NaiveDate> {
    let months = date.year() * 12 + date.month0() as i32 + delta;
    NaiveDate::from_ymd_opt(months.div_euclid(12), months.rem_euclid(12) as u32 + 1, 1)
        .filter(|d| (1..=9999).contains(&d.year()))
        .ok_or_else(|| anyhow::anyhow!("월 범위를 벗어났습니다"))
}
impl Journal {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.version == 1,
            "지원하지 않는 저장 버전: {}",
            self.version
        );
        let mut ids = HashSet::new();
        let mut names = HashSet::new();
        for c in &self.collections {
            ensure!(names.insert(c.name.trim()), "중복 컬렉션 이름");
            ensure!(
                ids.insert(c.id) && !c.id.is_nil() && !c.name.trim().is_empty(),
                "컬렉션 ID/이름이 올바르지 않습니다"
            );
        }
        for e in &self.entries {
            ensure!(ids.insert(e.id) && !e.id.is_nil(), "중복 또는 빈 항목 ID");
            ensure!(
                !e.text.trim().is_empty() && (1..=9999).contains(&e.date.year()),
                "항목 텍스트/날짜가 올바르지 않습니다"
            );
            self.validate_log(&e.log)?;
            ensure!(
                e.kind == Kind::Task || matches!(e.status, Status::Open | Status::Cancelled),
                "메모/이벤트에 작업 상태가 적용되었습니다"
            );
            ensure!(
                matches!(e.status, Status::Migrated | Status::Scheduled) == e.migrated_to.is_some(),
                "이월 상태와 연결이 다릅니다"
            );
            if let Some(id) = e.migrated_to {
                let target = self.entry(id)?;
                ensure!(
                    target.migrated_from == Some(e.id)
                        && target.id != e.id
                        && target.kind == Kind::Task
                        && !matches!(target.log, Log::Collection(_))
                        && e.kind == Kind::Task
                        && !same_location(e.date, &e.log, target.date, &target.log),
                    "잘못된 이월 대상 연결"
                );
                ensure!(
                    (e.status == Status::Scheduled) == (target.log == Log::Future),
                    "예약 상태와 대상 로그가 다릅니다"
                );
            }
            if let Some(id) = e.migrated_from {
                let source = self.entry(id)?;
                ensure!(
                    source.migrated_to == Some(e.id) && source.id != e.id,
                    "잘못된 이월 원본 연결"
                );
            }
        }
        for e in &self.entries {
            let mut visited = HashSet::new();
            let mut current = e;
            while let Some(id) = current.migrated_to {
                ensure!(visited.insert(current.id), "이월 연결에 순환이 있습니다");
                current = self.entry(id)?;
            }
        }
        Ok(())
    }
    fn validate_log(&self, log: &Log) -> Result<()> {
        if let Log::Collection(id) = log {
            ensure!(
                self.collections.iter().any(|c| c.id == *id),
                "컬렉션을 찾을 수 없습니다"
            );
        }
        Ok(())
    }
    pub fn entry(&self, id: Uuid) -> Result<&Entry> {
        self.entries
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| anyhow::anyhow!("항목을 찾을 수 없습니다"))
    }
    fn entry_mut(&mut self, id: Uuid) -> Result<&mut Entry> {
        self.entries
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or_else(|| anyhow::anyhow!("항목을 찾을 수 없습니다"))
    }
    pub fn add_collection(&mut self, name: &str) -> Result<Uuid> {
        let name = name.trim();
        ensure!(!name.is_empty(), "컬렉션 이름을 입력해 주세요");
        ensure!(
            !self.collections.iter().any(|c| c.name == name),
            "이미 있는 컬렉션 이름입니다"
        );
        let id = Uuid::new_v4();
        self.collections.push(Collection {
            id,
            name: name.to_owned(),
        });
        Ok(id)
    }
    pub fn add_entry(&mut self, date: NaiveDate, log: Log, kind: Kind, text: &str) -> Result<Uuid> {
        ensure!(!text.trim().is_empty(), "기록할 내용을 입력해 주세요");
        ensure!((1..=9999).contains(&date.year()), "잘못된 날짜입니다");
        self.validate_log(&log)?;
        let id = Uuid::new_v4();
        self.entries.push(Entry {
            id,
            date,
            log,
            kind,
            status: Status::Open,
            important: false,
            text: text.trim().to_owned(),
            migrated_from: None,
            migrated_to: None,
        });
        Ok(id)
    }
    pub fn set_status(&mut self, id: Uuid, status: Status) -> Result<()> {
        let e = self.entry_mut(id)?;
        ensure!(
            !matches!(e.status, Status::Migrated | Status::Scheduled),
            "이월한 원본은 변경할 수 없습니다"
        );
        ensure!(
            matches!(status, Status::Open | Status::Complete | Status::Cancelled),
            "이월 기능을 사용해 주세요"
        );
        ensure!(
            e.kind == Kind::Task || status == Status::Cancelled || status == Status::Open,
            "할 일만 완료할 수 있습니다"
        );
        e.status = status;
        Ok(())
    }
    pub fn edit_entry(&mut self, id: Uuid, text: &str) -> Result<()> {
        ensure!(!text.trim().is_empty(), "빈 기록은 저장할 수 없습니다");
        let e = self.entry_mut(id)?;
        ensure!(
            !matches!(e.status, Status::Migrated | Status::Scheduled),
            "이월 원본은 수정할 수 없습니다"
        );
        e.text = text.trim().to_owned();
        Ok(())
    }
    pub fn toggle_important(&mut self, id: Uuid) -> Result<()> {
        let e = self.entry_mut(id)?;
        ensure!(
            !matches!(e.status, Status::Migrated | Status::Scheduled),
            "이월 원본은 수정할 수 없습니다"
        );
        e.important = !e.important;
        Ok(())
    }
    pub fn migrate(&mut self, id: Uuid, date: NaiveDate, log: Log) -> Result<Uuid> {
        self.validate_log(&log)?;
        let source = self.entry(id)?.clone();
        ensure!(source.is_open_task(), "미완료 할 일만 이월할 수 있습니다");
        ensure!(
            !same_location(source.date, &source.log, date, &log),
            "다른 날짜 또는 로그를 선택해 주세요"
        );
        if matches!(log, Log::Collection(_)) {
            bail!("이월 대상은 일간/월간/미래 로그입니다");
        }
        let target = self.add_entry(date, log.clone(), Kind::Task, &source.text)?;
        let dest = self.entry_mut(target)?;
        dest.important = source.important;
        dest.migrated_from = Some(id);
        let original = self.entry_mut(id)?;
        original.status = if log == Log::Future {
            Status::Scheduled
        } else {
            Status::Migrated
        };
        original.migrated_to = Some(target);
        Ok(target)
    }
    pub fn visible(&self, date: NaiveDate, log: &Log) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|e| {
                &e.log == log
                    && match log {
                        Log::Daily => e.date == date,
                        Log::Monthly | Log::Future => {
                            e.date.year() == date.year() && e.date.month() == date.month()
                        }
                        Log::Collection(_) => true,
                    }
            })
            .collect()
    }
    pub fn search(&self, query: &str, filter: Filter) -> Vec<&Entry> {
        let q = query.trim().to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.text.to_lowercase().contains(&q)
                    && match filter {
                        Filter::All => true,
                        Filter::Open => e.status == Status::Open,
                        Filter::Complete => e.status == Status::Complete,
                    }
            })
            .collect()
    }
}

fn same_location(
    left_date: NaiveDate,
    left_log: &Log,
    right_date: NaiveDate,
    right_log: &Log,
) -> bool {
    left_log == right_log
        && match left_log {
            Log::Monthly | Log::Future => {
                left_date.year() == right_date.year() && left_date.month() == right_date.month()
            }
            Log::Daily => left_date == right_date,
            Log::Collection(_) => true,
        }
}
