//! 인메모리 워크플로 저장소 (단일 프로세스 · 테스트).
//!
//! SQL 저장소는 JSON 직렬화를 거치므로 호출자 메모리와 자연히 분리되지만,
//! 인메모리 구현은 **의도적으로 깊은 복사**를 해야 합니다. 그러지 않으면 저장한
//! 뒤 호출자가 `values` 를 바꾸면 저장된 상태까지 바뀝니다.

use super::{Error,
            Event,
            Run,
            Status,
            Store};
use std::{collections::HashMap,
          sync::Mutex};

#[derive(Debug, Default)]
pub struct MemoryStore {
    runs: Mutex<HashMap<String, Run>>,
    events: Mutex<Vec<Event>>,
}

impl MemoryStore {
    pub fn new() -> Self { Self::default() }
}

#[async_trait::async_trait]
impl Store for MemoryStore {
    async fn load(&self, run_id: &str) -> Result<Option<Run>, Error> {
        // 복사본을 돌려줍니다 — 호출자가 바꿔도 저장된 상태는 그대로입니다.
        Ok(self.runs.lock().unwrap().get(run_id).cloned())
    }

    async fn save(&self, run: &Run) -> Result<Run, Error> {
        let mut runs = self.runs.lock().unwrap();
        match runs.get(&run.id) {
            | Some(existing) => {
                // 낙관적 잠금 — 오래된 버전으로 덮어쓰면 완료 단계가 뒤로 돌아갑니다.
                if existing.version != run.version {
                    return Err(Error::VersionConflict);
                }
            },
            | None => {
                // 없는 run 을 0 이 아닌 버전으로 저장하려는 것은 프로그래머 오류이거나,
                // 누군가 지운 run 을 오래된 스냅샷으로 되살리려는 것입니다.
                if run.version != 0 {
                    return Err(Error::VersionConflict);
                }
            },
        }
        let mut stored = run.clone();
        stored.version += 1;
        runs.insert(stored.id.clone(), stored.clone());
        Ok(stored)
    }

    async fn list(&self, status: Option<Status>, limit: i64) -> Result<Vec<Run>, Error> {
        let limit = if limit <= 0 { 50 } else { limit } as usize;
        let runs = self.runs.lock().unwrap();
        let mut out: Vec<Run> = runs.values().filter(|r| status.map(|s| r.status == s).unwrap_or(true)).cloned().collect();
        out.sort_by_key(|r| std::cmp::Reverse(r.updated_at));
        out.truncate(limit);
        Ok(out)
    }

    async fn append_event(&self, e: &Event) -> Result<(), Error> {
        self.events.lock().unwrap().push(e.clone());
        Ok(())
    }

    async fn events(&self, run_id: &str) -> Result<Vec<Event>, Error> {
        Ok(self.events.lock().unwrap().iter().filter(|e| e.run_id == run_id).cloned().collect())
    }
}
