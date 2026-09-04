import type { components } from "./schema";

export type SpecSummary = components["schemas"]["SpecSummary"];
export type WorkflowRun = components["schemas"]["WorkflowRun"];
export type WorkflowOutcome = components["schemas"]["WorkflowOutcome"];
export type ControlEvent = components["schemas"]["ControlEvent"];
export type ReviewRequest = components["schemas"]["ReviewRequest"];
export type ReviewDecision = components["schemas"]["ReviewDecision"];
export type StartRunRequest = components["schemas"]["StartRunRequest"];
export type PreflightResponse = components["schemas"]["PreflightResponse"];
export type PathListing = components["schemas"]["PathListing"];
export type ApiProblem = components["schemas"]["ApiProblem"];

const TOKEN_KEY = "amap.api-token";
const ACTOR = "web-operator";

export function getApiToken(): string {
  return window.sessionStorage.getItem(TOKEN_KEY) ?? "";
}

export function setApiToken(token: string): void {
  if (token.trim()) {
    window.sessionStorage.setItem(TOKEN_KEY, token.trim());
  } else {
    window.sessionStorage.removeItem(TOKEN_KEY);
  }
}

function headers(json = false): HeadersInit {
  const token = getApiToken();
  return {
    ...(json ? { "Content-Type": "application/json" } : {}),
    ...(token ? { Authorization: `Bearer ${token}` } : {}),
    "X-AMAP-Actor": ACTOR
  };
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    ...init,
    headers: { ...headers(Boolean(init?.body)), ...init?.headers }
  });
  if (!response.ok) {
    const contentType = response.headers.get("content-type") ?? "";
    if (contentType.includes("application/json")) {
      const problem = (await response.json()) as Partial<ApiProblem>;
      throw new Error(problem.message || `요청에 실패했습니다. (${response.status})`);
    }
    const message = await response.text();
    throw new Error(message || `요청에 실패했습니다. (${response.status})`);
  }
  return response.json() as Promise<T>;
}

export const api = {
  listSpecs: () => request<SpecSummary[]>("/v1/specs"),
  listRuns: () => request<WorkflowRun[]>("/v1/runs"),
  getRun: (runId: string) => request<WorkflowRun>(`/v1/runs/${encodeURIComponent(runId)}`),
  listPaths: (purpose: "source" | "destination", parent = ".", search = "") => {
    const query = new URLSearchParams({ purpose, parent });
    if (search.trim()) query.set("search", search.trim());
    return request<PathListing>(`/v1/paths?${query.toString()}`);
  },
  preflightRun: (input: StartRunRequest) =>
    request<PreflightResponse>("/v1/runs/preflight", {
      method: "POST",
      body: JSON.stringify(input)
    }),
  startRun: (input: StartRunRequest) =>
    request<WorkflowRun>("/v1/runs", {
      method: "POST",
      body: JSON.stringify(input)
    }),
  listReviews: () => request<ReviewRequest[]>("/v1/reviews"),
  decideReview: (reviewId: string, decision: ReviewDecision) =>
    request<{ ok: boolean }>(`/v1/reviews/${encodeURIComponent(reviewId)}/decide`, {
      method: "POST",
      body: JSON.stringify(decision)
    }),
  resumeRun: (runId: string) =>
    request<WorkflowRun>(`/v1/runs/${encodeURIComponent(runId)}/resume`, { method: "POST" })
};

export function parseEventBlock(block: string): ControlEvent | null {
  const data = block
    .split("\n")
    .filter((line) => line.startsWith("data:"))
    .map((line) => line.slice(5).trimStart())
    .join("\n");
  if (!data) return null;
  try {
    return JSON.parse(data) as ControlEvent;
  } catch {
    return null;
  }
}

export async function streamRunEvents(
  runId: string,
  onEvent: (event: ControlEvent) => void,
  signal: AbortSignal
): Promise<void> {
  const response = await fetch(`/v1/runs/${encodeURIComponent(runId)}/events`, {
    headers: headers(),
    signal
  });
  if (!response.ok || !response.body) {
    throw new Error((await response.text()) || "이벤트 스트림에 연결하지 못했습니다.");
  }

  const reader = response.body.pipeThrough(new TextDecoderStream()).getReader();
  let buffer = "";
  while (!signal.aborted) {
    const { value, done } = await reader.read();
    if (done) break;
    buffer += value.replaceAll("\r\n", "\n");
    let boundary = buffer.indexOf("\n\n");
    while (boundary >= 0) {
      const event = parseEventBlock(buffer.slice(0, boundary));
      if (event) onEvent(event);
      buffer = buffer.slice(boundary + 2);
      boundary = buffer.indexOf("\n\n");
    }
  }
}
