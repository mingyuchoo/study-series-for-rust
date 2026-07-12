//! 워크플로 실행 엔진.

use super::{Definition,
            Definitions,
            Error,
            Event,
            Run,
            State,
            Status,
            Step,
            Store,
            WaitError,
            activity_key,
            def_key,
            event_type as ev,
            hash_input,
            hydrate_metadata,
            sync_metadata,
            validate_definition};
use crate::clock::{SharedClock,
                   SystemClock};
use anyhow::anyhow;
use serde_json::{Map,
                 Value};
use std::{sync::Arc,
          time::Duration};

/// lease 기본 유효 시간.
const DEFAULT_LEASE_TTL: Duration = Duration::from_secs(30);
/// 보상 전체에 주는 예산.
const COMPENSATION_BUDGET: Duration = Duration::from_secs(30);

/// 워크플로 엔진.
pub struct Engine {
    store: Arc<dyn Store>,
    worker: String,
    lease_ttl: Duration,
    clock: SharedClock,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.debug_struct("Engine").field("worker", &self.worker).finish() }
}

impl Engine {
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self {
            store,
            worker: new_worker_id(),
            lease_ttl: DEFAULT_LEASE_TTL,
            clock: Arc::new(SystemClock),
        }
    }

    pub fn with_worker(mut self, worker: impl Into<String>) -> Self {
        self.worker = worker.into();
        self
    }

    pub fn with_clock(mut self, clock: SharedClock) -> Self {
        self.clock = clock;
        self
    }

    pub fn store(&self) -> &Arc<dyn Store> { &self.store }

    async fn emit(&self, run_id: &str, r#type: &str, step: &str, attempt: i64, token: i64, msg: &str) {
        let e = Event {
            run_id: run_id.to_string(),
            at: self.clock.now(),
            r#type: r#type.to_string(),
            step: step.to_string(),
            attempt,
            worker: self.worker.clone(),
            fencing_token: token,
            message: msg.to_string(),
        };
        // 이벤트 기록 실패가 흐름을 막지는 않습니다 — 흐름 자체는 run 상태가
        // 결정합니다.
        if let Err(err) = self.store.append_event(&e).await {
            tracing::warn!("workflow: append_event failed: {err}");
        }
    }

    /// 흐름을 실행합니다. **이미 끝난 흐름을 다시 호출하면 저장된 결과만
    /// 돌아옵니다**(멱등).
    pub async fn execute(&self, def: &Definition, run_id: &str, input: Option<&Map<String, Value>>) -> Result<Run, Error> {
        validate_definition(def, run_id)?;
        let version = if def.version.is_empty() { "1".to_string() } else { def.version.clone() };
        let empty = Map::new();
        let input_hash = hash_input(input.unwrap_or(&empty));

        let mut run = match self.store.load(run_id).await? {
            | None => {
                let mut r = Run {
                    id: run_id.to_string(),
                    name: def.name.clone(),
                    status: Status::Running,
                    values: input.cloned().unwrap_or_default(),
                    started_at: Some(self.clock.now()),
                    definition_version: version.clone(),
                    input_hash: input_hash.clone(),
                    ..Default::default()
                };
                sync_metadata(&mut r);
                let r = self.store.save(&r).await?;
                self.emit(run_id, ev::WORKFLOW_STARTED, "", 0, 0, "").await;
                r
            },
            | Some(mut r) => {
                hydrate_metadata(&mut r);
                if r.definition_version.is_empty() {
                    r.definition_version = version.clone();
                }
                if r.input_hash.is_empty() {
                    r.input_hash = input_hash.clone();
                }
                // 같은 run ID 를 다른 정의로 재사용하면 완료 단계 목록의 의미가 달라집니다.
                if r.name != def.name || r.definition_version != version {
                    return Err(Error::Other(anyhow!(
                        "workflow definition mismatch: stored {}@{}, requested {}@{}",
                        r.name,
                        r.definition_version,
                        def.name,
                        version
                    )));
                }
                if input.is_some() && !r.input_hash.is_empty() && r.input_hash != input_hash {
                    return Err(Error::InputConflict);
                }
                r
            },
        };

        match run.status {
            // 멱등 — 돈이 두 번 나가지 않고 저장된 결과만 돌아옵니다.
            | Status::Completed => return Ok(run),
            | Status::Compensated | Status::Failed | Status::Cancelled => {
                return Err(Error::RunFailed(format!("run {run_id:?} ({}): {}", run.status, run.error)));
            },
            | Status::Waiting => {
                if let Some(next) = run.next_run_at
                    && next > self.clock.now()
                {
                    return Err(Error::Waiting(format!("until {}", crate::clock::to_rfc3339(next))));
                }
                run.status = Status::Running;
            },
            | Status::Running => {},
        }

        let mut state = State {
            run_id: run_id.to_string(),
            values: run.values.clone(),
            ..Default::default()
        };

        for step in &def.steps {
            // 완료된 단계는 건너뜁니다 — 이것이 "재개"입니다.
            if run.completed.iter().any(|c| c == &step.name) {
                continue;
            }

            // --- lease + fencing ---
            // 다른 worker 가 살아 있는 lease 를 쥐고 있으면 물러납니다.
            if !run.lease_owner.is_empty() && run.lease_owner != self.worker && run.lease_until.map(|t| t > self.clock.now()).unwrap_or(false) {
                return Err(Error::LeaseHeld);
            }
            run.current_step = step.name.clone();
            run.lease_owner = self.worker.clone();
            let lease_for = self.lease_ttl.max(worst_case_duration(step) + Duration::from_secs(5));
            run.lease_until = Some(self.clock.now() + chrono::Duration::from_std(lease_for).unwrap_or_default());
            run.fencing_token += 1;

            // **활동을 실행하기 전에 claim 을 영속화합니다.** 그래야 크래시 뒤에도
            // 누가 무엇을 하고 있었는지 알 수 있습니다.
            sync_metadata(&mut run);
            run = self.store.save(&run).await?;

            state.step_name = step.name.clone();
            state.activity_key = activity_key(run_id, run.recovery_count, &step.name);
            state.fencing_token = run.fencing_token;

            match self.execute_step(&run, step, &mut state).await {
                | Ok(()) => {},
                | Err(err) => {
                    // 단계가 "나중에 깨워달라"고 했다면 실패가 아닙니다.
                    if let Some(w) = err.downcast_ref::<WaitError>() {
                        run.status = Status::Waiting;
                        run.next_run_at = Some(w.until);
                        run.lease_owner.clear();
                        run.lease_until = None;
                        run.values = state.values.clone();
                        run.updated_at = Some(self.clock.now());
                        sync_metadata(&mut run);
                        run = self.store.save(&run).await?;
                        self.emit(run_id, ev::WORKFLOW_WAITING, &step.name, 0, run.fencing_token, &w.to_string()).await;
                        return Err(Error::Waiting(w.to_string()));
                    }
                    return self.rollback(def, run, state, &step.name, err).await;
                },
            }

            run.completed.push(step.name.clone());
            run.values = state.values.clone();
            run.updated_at = Some(self.clock.now());
            run.lease_owner.clear();
            run.current_step.clear();
            run.lease_until = None;
            // **단계마다 저장합니다** — 여기가 크래시 복구 지점입니다.
            sync_metadata(&mut run);
            run = self.store.save(&run).await?;
        }

        run.status = Status::Completed;
        run.values = state.values;
        run.updated_at = Some(self.clock.now());
        run.lease_owner.clear();
        run.current_step.clear();
        run.lease_until = None;
        sync_metadata(&mut run);
        self.store.save(&run).await
    }

    /// 한 단계를 재시도 정책에 따라 실행합니다.
    async fn execute_step(&self, run: &Run, step: &Step, state: &mut State) -> Result<(), anyhow::Error> {
        let attempts = step.retry.max_attempts.max(1);
        let mut backoff = if step.retry.initial_backoff.is_zero() {
            Duration::from_millis(100)
        } else {
            step.retry.initial_backoff
        };

        let mut last: Option<anyhow::Error> = None;
        for attempt in 1 ..= attempts {
            self.emit(&run.id, ev::STEP_STARTED, &step.name, attempt as i64, run.fencing_token, "").await;

            let result = if step.timeout.is_zero() {
                step.run.call(state).await
            } else {
                match tokio::time::timeout(step.timeout, step.run.call(state)).await {
                    | Ok(r) => r,
                    | Err(_) => Err(anyhow::Error::new(crate::transient::DeadlineExceeded)),
                }
            };

            match result {
                | Ok(()) => {
                    self.emit(&run.id, ev::STEP_COMPLETED, &step.name, attempt as i64, run.fencing_token, "").await;
                    return Ok(());
                },
                | Err(err) => {
                    self.emit(&run.id, ev::STEP_FAILED, &step.name, attempt as i64, run.fencing_token, &err.to_string())
                        .await;

                    // 대기 요청은 재시도 대상이 아닙니다 — 곧장 올려보냅니다.
                    if err.downcast_ref::<WaitError>().is_some() {
                        return Err(err);
                    }
                    // 업무 오류는 재시도해도 결과가 같습니다.
                    if !crate::transient::is_temporary(&err) {
                        return Err(err);
                    }
                    if attempt == attempts {
                        return Err(err);
                    }
                    last = Some(err);
                    let delay = if step.retry.max_backoff.is_zero() {
                        backoff
                    } else {
                        backoff.min(step.retry.max_backoff)
                    };
                    tokio::time::sleep(delay).await;
                    backoff *= 2;
                },
            }
        }
        Err(last.unwrap_or_else(|| anyhow!("workflow: step {:?} failed", step.name)))
    }

    /// 보상 — **완료된 단계를 역순으로** 되돌립니다.
    ///
    /// 하나가 실패해도 **멈추지 않고 계속합니다** — 일부라도 되돌리는 편이
    /// 낫습니다. 보상까지 실패하면 흐름을 `Failed` 로 표시해 **사람이
    /// 보게** 합니다.
    async fn rollback(&self, def: &Definition, mut run: Run, mut state: State, failed_step: &str, cause: anyhow::Error) -> Result<Run, Error> {
        let mut comp_errors: Vec<String> = Vec::new();
        let deadline = tokio::time::Instant::now() + COMPENSATION_BUDGET;

        // 역순(LIFO) — 가장 최근에 한 일부터 되돌립니다.
        for name in run.completed.iter().rev() {
            let Some(step) = def.steps.iter().find(|s| &s.name == name) else {
                continue;
            };
            let Some(comp) = &step.compensate else {
                continue; // 되돌릴 것이 없는 단계.
            };

            state.step_name = name.clone();
            // 보상도 같은 멱등 키 체계를 씁니다.
            state.activity_key = activity_key(&run.id, run.recovery_count, name);
            state.fencing_token = run.fencing_token;

            self.emit(&run.id, ev::COMPENSATION_STARTED, name, 0, run.fencing_token, "").await;

            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            let result = if remaining.is_zero() {
                Err(anyhow!("compensation budget exhausted"))
            } else {
                match tokio::time::timeout(remaining, comp.call(&mut state)).await {
                    | Ok(r) => r,
                    | Err(_) => Err(anyhow!("compensation timed out")),
                }
            };

            match result {
                | Ok(()) => {
                    self.emit(&run.id, ev::COMPENSATION_COMPLETED, name, 0, run.fencing_token, "").await;
                },
                | Err(e) => {
                    comp_errors.push(format!("compensate {name:?}: {e}"));
                    self.emit(&run.id, ev::COMPENSATION_FAILED, name, 0, run.fencing_token, &e.to_string()).await;
                    // 계속합니다 — 일부라도 되돌리는 편이 낫습니다.
                },
            }
        }

        run.values = state.values;
        run.error = format!("step {failed_step:?}: {cause}");
        run.updated_at = Some(self.clock.now());
        run.status = Status::Compensated;
        run.lease_owner.clear();
        run.current_step.clear();
        run.lease_until = None;

        if !comp_errors.is_empty() {
            // 되돌리지 못했습니다 — 사람이 봐야 합니다.
            run.status = Status::Failed;
            run.compensate_error = comp_errors.join("; ");
        }

        sync_metadata(&mut run);
        // 상태를 먼저 남깁니다 — 보상 결과를 잃으면 운영자가 무엇이 되돌려졌는지 알 수
        // 없습니다.
        self.store.save(&run).await?;

        // 보상이 성공했더라도 **원래 호출은 실패한 것**이므로 오류를 돌려줍니다.
        Err(Error::Failed(format!("workflow {:?} step {failed_step:?}: {cause}", def.name)))
    }

    /// 실패·보상·대기 상태의 흐름을 다시 실행 가능하게 만듭니다.
    ///
    /// `Failed`/`Compensated` 에서 복구하면 **완료 단계를 전부 비우고**
    /// 처음부터 다시 실행합니다(이미 보상으로 되돌렸으므로). 그리고
    /// `recovery_count` 를 올려 **멱등 키의 세대를 바꿉니다** — 외부
    /// 시스템이 이전 시도와 구분할 수 있게 하기 위함입니다.
    pub async fn recover(&self, run_id: &str) -> Result<Run, Error> {
        let mut run = self
            .store
            .load(run_id)
            .await?
            .ok_or_else(|| Error::Other(anyhow!("workflow: run {run_id:?} not found")))?;
        hydrate_metadata(&mut run);

        if !matches!(run.status, Status::Failed | Status::Compensated | Status::Waiting) {
            return Err(Error::Other(anyhow!("workflow: run {run_id:?} is not recoverable from {}", run.status)));
        }

        if matches!(run.status, Status::Failed | Status::Compensated) {
            run.completed.clear();
            run.recovery_count += 1;
        }
        run.status = Status::Running;
        run.error.clear();
        run.compensate_error.clear();
        run.next_run_at = None;
        run.lease_owner.clear();
        run.lease_until = None;

        sync_metadata(&mut run);
        let saved = self.store.save(&run).await?;
        self.emit(run_id, ev::WORKFLOW_RECOVERED, "", 0, saved.fencing_token, "").await;
        Ok(saved)
    }

    /// 흐름을 취소합니다. 대기 중인 흐름도 취소할 수 있습니다.
    pub async fn cancel(&self, run_id: &str, reason: &str) -> Result<Run, Error> {
        let mut run = self
            .store
            .load(run_id)
            .await?
            .ok_or_else(|| Error::Other(anyhow!("workflow: run {run_id:?} not found")))?;
        hydrate_metadata(&mut run);

        if run.status.terminal() {
            return Err(Error::Other(anyhow!("workflow: run {run_id:?} is already terminal")));
        }
        run.status = Status::Cancelled;
        run.error = reason.to_string();
        run.updated_at = Some(self.clock.now());
        run.lease_owner.clear();
        run.lease_until = None;

        sync_metadata(&mut run);
        let saved = self.store.save(&run).await?;
        self.emit(run_id, ev::WORKFLOW_CANCELLED, "", 0, saved.fencing_token, reason).await;
        Ok(saved)
    }
}

fn new_worker_id() -> String {
    use rand::Rng as _;
    let mut b = [0u8; 4];
    rand::rng().fill_bytes(&mut b);
    format!("worker-{}", hex::encode(b))
}

/// 단계가 최악의 경우 얼마나 걸릴지 — lease 를 그만큼 늘려 다른 worker 가
/// 재시도 도중에 lease 를 훔쳐가지 않게 합니다.
fn worst_case_duration(step: &Step) -> Duration {
    let attempts = step.retry.max_attempts.max(1);
    let mut total = step.timeout * attempts;
    let mut backoff = if step.retry.initial_backoff.is_zero() {
        Duration::from_millis(100)
    } else {
        step.retry.initial_backoff
    };
    for _ in 0 .. attempts {
        let d = if step.retry.max_backoff.is_zero() {
            backoff
        } else {
            backoff.min(step.retry.max_backoff)
        };
        total += d;
        backoff *= 2;
    }
    total
}

/// 대기 중인 흐름을 때가 되면 재개합니다.
pub struct Scheduler {
    engine: Arc<Engine>,
    interval: Duration,
    definitions: Definitions,
}

impl Scheduler {
    pub fn new(engine: Arc<Engine>, interval: Duration) -> Self {
        Self {
            engine,
            interval: if interval.is_zero() { Duration::from_secs(1) } else { interval },
            definitions: Definitions::new(),
        }
    }

    pub fn register(&mut self, def: Arc<Definition>) { self.definitions.insert(def_key(&def.name, &def.version), def); }

    /// 취소될 때까지 주기적으로 깨워 due 인 흐름을 재개합니다.
    pub async fn run(&self, cancel: tokio_util_stub::CancelToken) {
        let mut ticker = tokio::time::interval(self.interval);
        loop {
            tokio::select! {
                _ = ticker.tick() => { self.tick().await; }
                _ = cancel.cancelled() => return,
            }
        }
    }

    async fn tick(&self) {
        let Ok(runs) = self.engine.store.list(Some(Status::Waiting), 100).await else {
            return;
        };
        let now = self.engine.clock.now();
        for mut run in runs {
            hydrate_metadata(&mut run);
            if run.next_run_at.map(|t| t > now).unwrap_or(false) {
                continue; // 아직 때가 아닙니다.
            }
            let Some(def) = self.definitions.get(&def_key(&run.name, &run.definition_version)) else {
                continue; // 등록되지 않은 정의는 건드리지 않습니다.
            };
            // input=None 이면 입력 해시 대조를 건너뜁니다 (재개이므로).
            if let Err(e) = self.engine.execute(def, &run.id, None).await {
                tracing::debug!("workflow: scheduler resume {}: {e}", run.id);
            }
        }
    }
}

/// 아주 작은 취소 토큰 (tokio-util 을 끌어오지 않기 위함).
pub mod tokio_util_stub {
    use std::sync::Arc;
    use tokio::sync::Notify;

    #[derive(Clone, Default)]
    pub struct CancelToken(Arc<Notify>);

    impl CancelToken {
        pub fn new() -> Self { Self(Arc::new(Notify::new())) }

        pub fn cancel(&self) { self.0.notify_waiters(); }

        pub async fn cancelled(&self) { self.0.notified().await; }
    }
}
