import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Activity,
  AlertTriangle,
  Check,
  CheckCircle2,
  ChevronRight,
  Circle,
  Clock3,
  KeyRound,
  LoaderCircle,
  Pause,
  Play,
  RefreshCw,
  ShieldCheck,
  TerminalSquare,
  X,
  XCircle
} from "lucide-react";
import { Route, Routes, useNavigate, useParams } from "react-router";
import {
  api,
  getApiToken,
  setApiToken,
  streamRunEvents,
  type ControlEvent,
  type ReviewRequest,
  type SpecSummary,
  type WorkflowRun
} from "./api/client";

const PHASES = [
  "discovery",
  "rule_mining",
  "behavior_mining",
  "uncertainty",
  "architecture",
  "test_generation",
  "boundary",
  "build",
  "adversarial",
  "verify",
  "rca",
  "fix",
  "review"
] as const;

const PHASE_LABELS: Record<string, string> = {
  discovery: "소스 발견",
  rule_mining: "규칙 마이닝",
  behavior_mining: "동작 마이닝",
  uncertainty: "불확실성 평가",
  architecture: "아키텍처 설계",
  test_generation: "테스트 생성",
  boundary: "경계값 생성",
  build: "구현 생성",
  adversarial: "적대적 시나리오",
  verify: "독립 검증",
  rca: "근본 원인 분석",
  fix: "수정 생성",
  review: "독립 리뷰"
};

const STATUS_LABELS: Record<string, string> = {
  running: "실행 중",
  halted: "검토 대기",
  certified: "인증 완료",
  failed: "실패",
  error: "오류"
};

function formatDate(value: string): string {
  return new Intl.DateTimeFormat("ko-KR", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit"
  }).format(new Date(value));
}

function statusTone(status: string): string {
  if (status === "certified" || status === "approved") return "success";
  if (status === "failed" || status === "error" || status === "rejected") return "danger";
  if (status === "halted" || status === "pending") return "warning";
  return "active";
}

function StatusBadge({ status }: { status: string }) {
  return (
    <span className={`status-badge ${statusTone(status)}`}>
      <span className="status-dot" />
      {STATUS_LABELS[status] ?? status}
    </span>
  );
}

function useRunEvents(runId: string | undefined, active: boolean) {
  const queryClient = useQueryClient();
  const [events, setEvents] = useState<ControlEvent[]>([]);
  const [connected, setConnected] = useState(false);

  useEffect(() => {
    setEvents([]);
    setConnected(false);
    if (!runId) return;

    const controller = new AbortController();
    let retry: ReturnType<typeof setTimeout> | undefined;
    const connect = async () => {
      try {
        await streamRunEvents(
          runId,
          (event) => {
            setConnected(true);
            setEvents((current) => {
              const key = `${event.at}:${event.subject}`;
              if (current.some((item) => `${item.at}:${item.subject}` === key)) return current;
              return [...current, event].slice(-200);
            });
            void queryClient.invalidateQueries({ queryKey: ["run", runId] });
            void queryClient.invalidateQueries({ queryKey: ["runs"] });
            if (event.subject.startsWith("human.review")) {
              void queryClient.invalidateQueries({ queryKey: ["reviews"] });
            }
          },
          controller.signal
        );
      } catch {
        setConnected(false);
        if (!controller.signal.aborted && active) retry = setTimeout(connect, 1_500);
      }
    };
    void connect();
    return () => {
      controller.abort();
      if (retry) clearTimeout(retry);
    };
  }, [active, queryClient, runId]);

  return { events, connected };
}

function RunList({ runs, selectedId }: { runs: WorkflowRun[]; selectedId?: string }) {
  const navigate = useNavigate();
  const sorted = [...runs].sort(
    (left, right) => new Date(right.started_at).getTime() - new Date(left.started_at).getTime()
  );

  if (!sorted.length) {
    return <p className="empty-copy">아직 실행 기록이 없습니다.</p>;
  }

  return (
    <div className="run-list">
      {sorted.slice(0, 8).map((run) => (
        <button
          className={`run-row ${run.id === selectedId ? "selected" : ""}`}
          key={run.id}
          onClick={() => navigate(`/runs/${run.id}`)}
          type="button"
        >
          <span className="run-row-main">
            <strong>{run.function_id}</strong>
            <small>{formatDate(run.started_at)}</small>
          </span>
          <StatusBadge status={run.status} />
          <ChevronRight aria-hidden="true" size={17} />
        </button>
      ))}
    </div>
  );
}

function NewRunPanel({ specs }: { specs: SpecSummary[] }) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [specPath, setSpecPath] = useState("");
  const [mock, setMock] = useState(false);
  const selected = specs.find((spec) => spec.path === specPath);

  useEffect(() => {
    if (!specPath && specs[0]) setSpecPath(specs[0].path);
  }, [specPath, specs]);

  const start = useMutation({
    mutationFn: () => api.startRun(specPath, mock),
    onSuccess: (run) => {
      void queryClient.invalidateQueries({ queryKey: ["runs"] });
      navigate(`/runs/${run.id}`);
    }
  });

  return (
    <section className="launch-panel" aria-labelledby="launch-title">
      <div className="section-heading compact">
        <div>
          <p className="eyebrow">새 워크플로</p>
          <h2 id="launch-title">현대화 실행</h2>
        </div>
        <Play aria-hidden="true" size={18} />
      </div>
      <label className="field-label" htmlFor="run-spec">
        실행 명세
      </label>
      <select id="run-spec" value={specPath} onChange={(event) => setSpecPath(event.target.value)}>
        {specs.map((spec) => (
          <option key={spec.path} value={spec.path}>
            {spec.name}
          </option>
        ))}
      </select>
      {selected ? (
        <div className="spec-summary">
          <span className={`priority ${selected.priority.toLowerCase()}`}>{selected.priority}</span>
          <span>{selected.domain}</span>
          <span>{selected.function_id}</span>
        </div>
      ) : null}
      {selected?.mock_available ? (
        <label className="checkbox-row">
          <input checked={mock} onChange={(event) => setMock(event.target.checked)} type="checkbox" />
          fixture 기반 모의 LLM 사용
        </label>
      ) : null}
      <button
        className="primary-button"
        disabled={!specPath || start.isPending}
        onClick={() => start.mutate()}
        type="button"
      >
        {start.isPending ? <LoaderCircle className="spin" size={18} /> : <Play size={18} />}
        실행 시작
      </button>
      {start.error ? <p className="inline-error">{start.error.message}</p> : null}
    </section>
  );
}

function phaseState(
  phase: string,
  run: WorkflowRun,
  events: ControlEvent[]
): "done" | "active" | "failed" | "halted" | "queued" {
  const relevant = events.filter((event) => event.subject.startsWith(`agent.${phase}.`));
  if (relevant.some((event) => event.subject.endsWith("failed"))) return "failed";
  if (relevant.some((event) => event.subject.endsWith("halted"))) return "halted";
  if (relevant.some((event) => event.subject.endsWith("completed"))) return "done";
  if (phase in run.checkpoint) return "done";
  if (relevant.some((event) => event.subject.endsWith("started"))) return "active";
  return "queued";
}

function PhaseTimeline({ run, events }: { run: WorkflowRun; events: ControlEvent[] }) {
  return (
    <ol className="phase-timeline">
      {PHASES.map((phase, index) => {
        const state = phaseState(phase, run, events);
        const Icon =
          state === "done"
            ? Check
            : state === "active"
              ? LoaderCircle
              : state === "failed"
                ? X
                : state === "halted"
                  ? Pause
                  : Circle;
        return (
          <li className={`phase ${state}`} key={phase}>
            <span className="phase-index">{String(index + 1).padStart(2, "0")}</span>
            <span className="phase-icon">
              <Icon className={state === "active" ? "spin" : ""} size={15} />
            </span>
            <span className="phase-name">{PHASE_LABELS[phase]}</span>
          </li>
        );
      })}
    </ol>
  );
}

function GatePanel({ run }: { run: WorkflowRun }) {
  if (run.outcome && "error" in run.outcome) {
    return (
      <section className="gate-panel blocked">
        <div className="gate-title">
          <XCircle size={26} />
          <div>
            <p className="eyebrow">WORKFLOW ERROR</p>
            <h3>실행을 완료하지 못했습니다.</h3>
            <p className="gate-error-message">{run.outcome.error}</p>
          </div>
        </div>
      </section>
    );
  }

  const gate = run.outcome?.gate;
  if (!gate) {
    return (
      <section className="gate-panel awaiting">
        <ShieldCheck aria-hidden="true" size={25} />
        <div>
          <h3>품질 게이트 대기 중</h3>
          <p>독립 검증이 끝나면 기능 동등성 기준별 결과가 표시됩니다.</p>
        </div>
      </section>
    );
  }

  return (
    <section className={`gate-panel ${gate.certified ? "certified" : "blocked"}`}>
      <div className="gate-title">
        {gate.certified ? <CheckCircle2 size={26} /> : <XCircle size={26} />}
        <div>
          <p className="eyebrow">결정론적 품질 게이트</p>
          <h3>{gate.certified ? "기능 동등성 인증 완료" : "인증 기준 미충족"}</h3>
        </div>
      </div>
      <div className="gate-grid">
        {gate.checks.map((check) => (
          <div className={`gate-check ${check.passed ? "pass" : "fail"}`} key={check.kpi}>
            {check.passed ? <Check size={16} /> : <X size={16} />}
            <span>{check.kpi}</span>
            <strong>{check.actual}</strong>
            <small>기준 {check.required}</small>
          </div>
        ))}
      </div>
    </section>
  );
}

function ReviewCard({ review, onSettled }: { review: ReviewRequest; onSettled: () => void }) {
  const decide = useMutation({
    mutationFn: (status: "approved" | "rejected") => api.decideReview(review.id, { status }),
    onSuccess: onSettled
  });

  return (
    <article className="review-card">
      <div className="review-card-head">
        <span className="review-tier">{review.tier.replaceAll("_", " ")}</span>
        <StatusBadge status={review.status} />
      </div>
      <strong>{review.function_id}</strong>
      <p>{review.reason}</p>
      <div className="uncertainty">
        <span>불확실성</span>
        <strong>{(review.uncertainty * 100).toFixed(2)}%</strong>
      </div>
      {review.status === "pending" ? (
        <div className="review-actions">
          <button disabled={decide.isPending} onClick={() => decide.mutate("rejected")} type="button">
            거절
          </button>
          <button
            className="approve"
            disabled={decide.isPending}
            onClick={() => decide.mutate("approved")}
            type="button"
          >
            승인
          </button>
        </div>
      ) : null}
      {decide.error ? <p className="inline-error">{decide.error.message}</p> : null}
    </article>
  );
}

function Console() {
  const { runId } = useParams();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [token, setToken] = useState(getApiToken());
  const [showToken, setShowToken] = useState(false);

  const specs = useQuery({ queryKey: ["specs"], queryFn: api.listSpecs });
  const runs = useQuery({ queryKey: ["runs"], queryFn: api.listRuns, refetchInterval: 5_000 });
  const reviews = useQuery({ queryKey: ["reviews"], queryFn: api.listReviews, refetchInterval: 5_000 });

  useEffect(() => {
    if (!runId && runs.data?.length) {
      const latest = [...runs.data].sort(
        (left, right) =>
          new Date(right.started_at).getTime() - new Date(left.started_at).getTime()
      )[0];
      navigate(`/runs/${latest.id}`, { replace: true });
    }
  }, [navigate, runId, runs.data]);

  const run = useQuery({
    queryKey: ["run", runId],
    queryFn: () => api.getRun(runId!),
    enabled: Boolean(runId),
    refetchInterval: (query) => (query.state.data?.status === "running" ? 3_000 : false)
  });
  const { events, connected } = useRunEvents(runId, run.data?.status === "running");
  const selectedReviews = useMemo(
    () => (reviews.data ?? []).filter((review) => !runId || review.run_id === runId),
    [reviews.data, runId]
  );
  const pendingReviews = (reviews.data ?? []).filter((review) => review.status === "pending").length;
  const certifiedRuns = (runs.data ?? []).filter((item) => item.status === "certified").length;

  const resume = useMutation({
    mutationFn: () => api.resumeRun(runId!),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["run", runId] });
      void queryClient.invalidateQueries({ queryKey: ["runs"] });
    }
  });

  const saveToken = () => {
    setApiToken(token);
    setShowToken(false);
    void queryClient.invalidateQueries();
  };

  const connectionError = specs.error ?? runs.error ?? reviews.error;

  return (
    <div className="app-shell">
      <header className="topbar">
        <div className="brand-mark">A</div>
        <div className="brand-copy">
          <strong>AMAP</strong>
          <span>Modernization Assurance</span>
        </div>
        <div className="topbar-spacer" />
        <div className={`connection ${connectionError ? "offline" : "online"}`}>
          <Activity size={15} />
          {connectionError ? "API 연결 필요" : "Control Plane 연결됨"}
        </div>
        <button className="icon-button" onClick={() => setShowToken((value) => !value)} title="API 토큰" type="button">
          <KeyRound size={18} />
        </button>
      </header>

      {showToken ? (
        <div className="token-bar">
          <label htmlFor="api-token">세션 토큰 (서비스 토큰 또는 OIDC ID 토큰)</label>
          <input
            id="api-token"
            onChange={(event) => setToken(event.target.value)}
            placeholder="개발 모드에서는 비워 두십시오"
            type="password"
            value={token}
          />
          <button onClick={saveToken} type="button">적용</button>
        </div>
      ) : null}

      <aside className="sidebar">
        <div className="system-summary">
          <p className="eyebrow">CONTROL PLANE</p>
          <h1>현대화 운영 콘솔</h1>
          <div className="summary-metrics">
            <div>
              <span>전체 실행</span>
              <strong>{runs.data?.length ?? 0}</strong>
            </div>
            <div>
              <span>인증 완료</span>
              <strong>{certifiedRuns}</strong>
            </div>
            <div>
              <span>검토 대기</span>
              <strong className={pendingReviews ? "warn" : ""}>{pendingReviews}</strong>
            </div>
          </div>
        </div>

        {specs.data ? <NewRunPanel specs={specs.data} /> : null}
        {connectionError ? (
          <div className="connection-error">
            <AlertTriangle size={18} />
            <div>
              <strong>Control Plane에 연결할 수 없습니다.</strong>
              <p>{connectionError.message}</p>
            </div>
          </div>
        ) : null}

        <section className="recent-runs" aria-labelledby="recent-title">
          <div className="section-heading compact">
            <div>
              <p className="eyebrow">RECENT</p>
              <h2 id="recent-title">최근 실행</h2>
            </div>
            {runs.isFetching ? <RefreshCw className="spin" size={15} /> : null}
          </div>
          <RunList runs={runs.data ?? []} selectedId={runId} />
        </section>
      </aside>

      <main className="workspace">
        {run.data ? (
          <>
            <section className="run-hero">
              <div>
                <div className="run-kicker">
                  <StatusBadge status={run.data.status} />
                  <span className={`stream-state ${connected ? "connected" : ""}`}>
                    <span /> {connected ? "실시간 이벤트 연결" : "이벤트 연결 중"}
                  </span>
                </div>
                <h2>{run.data.function_id}</h2>
                <p className="run-id">{run.data.id}</p>
              </div>
              <div className="run-meta">
                <span>
                  <Clock3 size={16} /> 시작 {formatDate(run.data.started_at)}
                </span>
                <span>
                  <TerminalSquare size={16} /> {run.data.mock ? "모의 LLM" : "실제 LLM"}
                </span>
              </div>
            </section>

            {run.data.status === "halted" && selectedReviews.some((review) => review.status === "approved") ? (
              <div className="resume-banner">
                <div>
                  <strong>필수 검토가 승인되었습니다.</strong>
                  <span>중단된 지점의 체크포인트부터 실행을 이어갈 수 있습니다.</span>
                </div>
                <button disabled={resume.isPending} onClick={() => resume.mutate()} type="button">
                  {resume.isPending ? <LoaderCircle className="spin" size={18} /> : <Play size={18} />}
                  실행 재개
                </button>
              </div>
            ) : null}

            <div className="workspace-grid">
              <section className="pipeline-card">
                <div className="section-heading">
                  <div>
                    <p className="eyebrow">PIPELINE</p>
                    <h2>에이전트 실행 흐름</h2>
                  </div>
                  <span className="event-count">{events.length} events</span>
                </div>
                <PhaseTimeline events={events} run={run.data} />
              </section>

              <section className="review-column">
                <div className="section-heading">
                  <div>
                    <p className="eyebrow">HUMAN REVIEW</p>
                    <h2>검토 요청</h2>
                  </div>
                  <span className="count-pill">{selectedReviews.length}</span>
                </div>
                {selectedReviews.length ? (
                  selectedReviews.map((review) => (
                    <ReviewCard
                      key={review.id}
                      onSettled={() => {
                        void queryClient.invalidateQueries({ queryKey: ["reviews"] });
                        void queryClient.invalidateQueries({ queryKey: ["run", runId] });
                      }}
                      review={review}
                    />
                  ))
                ) : (
                  <div className="empty-review">
                    <ShieldCheck size={23} />
                    <p>이 실행에는 처리할 검토 요청이 없습니다.</p>
                  </div>
                )}
              </section>
            </div>
            <GatePanel run={run.data} />
          </>
        ) : runId && run.isLoading ? (
          <div className="loading-state">
            <LoaderCircle className="spin" size={28} />
            실행 상태를 불러오고 있습니다.
          </div>
        ) : (
          <div className="welcome-state">
            <div className="welcome-icon">
              <ShieldCheck size={34} />
            </div>
            <p className="eyebrow">EVIDENCE-DRIVEN MODERNIZATION</p>
            <h2>실행할 현대화 명세를 선택하십시오.</h2>
            <p>발견부터 기능 동등성 인증까지 모든 단계와 검토 요청을 한 화면에서 추적합니다.</p>
          </div>
        )}
      </main>
    </div>
  );
}

export default function App() {
  return (
    <Routes>
      <Route element={<Console />} path="/" />
      <Route element={<Console />} path="/runs/:runId" />
      <Route element={<Console />} path="*" />
    </Routes>
  );
}
