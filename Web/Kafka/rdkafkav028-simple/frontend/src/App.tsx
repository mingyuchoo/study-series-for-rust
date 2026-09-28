import { For, Show, createSignal, onCleanup, onMount } from "solid-js";

type KafkaMessage = {
  topic: string;
  key: string | null;
  payload: string | null;
  partition: number;
  offset: string;
  timestamp: number | null;
};
type Delivery = { topic: string; partition: number; offset: string };
type Connection = "idle" | "connecting" | "live" | "error";

const topicPattern = "[a-zA-Z0-9._-]{1,249}";
const validTopic = (value: string) =>
  new RegExp(`^${topicPattern}$`).test(value) &&
  value !== "." &&
  value !== "..";
const errorText = (error: unknown) =>
  error instanceof Error ? error.message : String(error);

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    ...init,
    signal: AbortSignal.timeout(15000),
  });
  const text = await response.text();
  let data;
  try {
    data = JSON.parse(text);
  } catch {
    throw new Error(text || `HTTP ${response.status}`);
  }
  if (!response.ok) throw new Error(data.error || `HTTP ${response.status}`);
  return data as T;
}

export default function App() {
  const [topic, setTopic] = createSignal("rust");
  const [key, setKey] = createSignal("hello-solid");
  const [payload, setPayload] = createSignal(
    '{\n  "message": "안녕하세요, Kafka!",\n  "from": "SolidJS"\n}',
  );
  const [position, setPosition] = createSignal("latest");
  const [connection, setConnection] = createSignal<Connection>("idle");
  const [streamError, setStreamError] = createSignal("");
  const [produceError, setProduceError] = createSignal("");
  const [sending, setSending] = createSignal(false);
  const [delivery, setDelivery] = createSignal<Delivery>();
  const [messages, setMessages] = createSignal<KafkaMessage[]>([]);
  const [received, setReceived] = createSignal(0);
  const [sent, setSent] = createSignal(0);
  const [health, setHealth] = createSignal("확인 중");
  const [healthy, setHealthy] = createSignal(false);
  const [checking, setChecking] = createSignal(false);
  const [brokers, setBrokers] = createSignal("Kafka broker");
  let source: EventSource | undefined;
  const active = () => connection() === "connecting" || connection() === "live";
  const connectionLabel = () =>
    ({
      idle: "대기 중",
      connecting: "연결 중",
      live: "수신 중",
      error: "연결 오류",
    })[connection()];

  async function checkHealth() {
    setChecking(true);
    try {
      const result = await request<{ brokers: string }>("/api/health");
      setBrokers(result.brokers);
      setHealth("브로커 연결됨");
      setHealthy(true);
    } catch (error) {
      setHealth(`연결 실패: ${errorText(error)}`);
      setHealthy(false);
    } finally {
      setChecking(false);
    }
  }

  function stop() {
    source?.close();
    source = undefined;
    setConnection("idle");
  }

  function start() {
    stop();
    setStreamError("");
    if (!validTopic(topic())) {
      setStreamError(
        "유효한 토픽 이름을 입력하세요. 영문, 숫자, . _ - 사용 가능 (. 및 .. 제외)",
      );
      return;
    }
    setConnection("connecting");
    const current = new EventSource(
      `/api/events?${new URLSearchParams({ topic: topic(), from: position() })}`,
    );
    source = current;
    const fail = (message: string) => {
      if (source !== current) return;
      current.close();
      source = undefined;
      setConnection("error");
      setStreamError(message);
    };
    current.addEventListener("ready", () => {
      if (source === current) setConnection("live");
    });
    current.addEventListener("message", (event: MessageEvent<string>) => {
      if (source !== current) return;
      const message = JSON.parse(event.data) as KafkaMessage;
      setMessages((previous) => [message, ...previous].slice(0, 200));
      setReceived((count) => count + 1);
    });
    current.addEventListener("kafka-error", (event) => {
      fail(JSON.parse((event as MessageEvent<string>).data).error);
    });
    current.onerror = () =>
      fail(
        "스트림 연결이 끊어졌습니다. 서버와 Kafka를 확인한 뒤 수신 시작을 눌러주세요.",
      );
  }

  async function produce(event: SubmitEvent) {
    event.preventDefault();
    if (sending()) return;
    setProduceError("");
    setDelivery(undefined);
    if (!validTopic(topic())) {
      setProduceError("유효한 토픽 이름을 입력하세요.");
      return;
    }
    const encoder = new TextEncoder();
    if (
      encoder.encode(payload()).length > 256 * 1024 ||
      encoder.encode(key()).length > 4096
    ) {
      setProduceError("메시지는 UTF-8 256 KiB, 키는 4 KiB 이하여야 합니다.");
      return;
    }
    setSending(true);
    try {
      setDelivery(
        await request<Delivery>("/api/messages", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            topic: topic(),
            key: key() || null,
            payload: payload(),
          }),
        }),
      );
      setSent((count) => count + 1);
    } catch (error) {
      setProduceError(errorText(error));
    } finally {
      setSending(false);
    }
  }

  onMount(() => {
    void checkHealth();
  });
  onCleanup(() => source?.close());

  return (
    <div class="shell">
      <header class="topbar">
        <a class="brand" href="/" aria-label="Kafka Playground 홈">
          <span class="brand-icon">K</span>
          <span>
            Kafka <b>Playground</b>
          </span>
        </a>
        <span class="stack">
          SolidJS <span>+</span> Rust
        </span>
      </header>
      <main>
        <div class="intro">
          <div>
            <p class="eyebrow">MESSAGING LAB / 01</p>
            <h1>메시지가 흐르는 곳.</h1>
            <p class="subtitle">
              메시지를 보내고, Kafka를 통해 도착하는 순간을 확인하세요.
            </p>
          </div>
          <span class="local-tag">LOCAL WORKSPACE</span>
        </div>
        <section class="connection-panel" aria-label="연결 설정">
          <div class="broker">
            <span classList={{ dot: true, green: healthy() }} />
            <div>
              <span class="small-label">BOOTSTRAP SERVER</span>
              <strong>{brokers()}</strong>
            </div>
          </div>
          <div class="topic-control">
            <label for="topic">Topic</label>
            <input
              id="topic"
              value={topic()}
              onInput={(e) => setTopic(e.currentTarget.value)}
              disabled={active() || sending()}
              maxLength={249}
              pattern={topicPattern}
              required
            />
          </div>
          <button
            class="button ghost"
            disabled={checking()}
            onClick={checkHealth}
          >
            {checking() ? "확인 중…" : "연결 확인 ↗"}
          </button>
          <p
            classList={{ "health-message": true, "text-error": !healthy() }}
            role="status"
          >
            {health()}
          </p>
        </section>
        <div class="workspace">
          <section class="panel producer-panel">
            <div class="panel-heading">
              <div>
                <p class="eyebrow">01 / PRODUCE</p>
                <h2>메시지 발행</h2>
              </div>
              <span class="icon-tile">↗</span>
            </div>
            <form onSubmit={produce}>
              <label for="message-key">
                메시지 키 <span class="optional">선택 사항</span>
              </label>
              <input
                id="message-key"
                value={key()}
                onInput={(e) => setKey(e.currentTarget.value)}
                placeholder="키가 없으면 비워두세요"
              />
              <p class="field-hint">같은 키는 같은 파티션으로 전달됩니다.</p>
              <div class="label-row">
                <label for="payload">메시지 본문</label>
                <span class="code-tag">TEXT / JSON</span>
              </div>
              <textarea
                id="payload"
                value={payload()}
                onInput={(e) => setPayload(e.currentTarget.value)}
                spellcheck={false}
              />
              <div class="editor-footer">
                <span>UTF-8 · 최대 256 KiB</span>
                <span>
                  {new TextEncoder().encode(payload()).length.toLocaleString()}{" "}
                  bytes
                </span>
              </div>
              <button
                class="button primary send-button"
                type="submit"
                disabled={sending()}
              >
                {sending() ? "Kafka 응답 대기 중…" : "메시지 발행"}
                <span>↗</span>
              </button>
              <Show when={delivery()}>
                {(result) => (
                  <p class="notice success" role="status">
                    발행 완료 · {result().topic} · 파티션 {result().partition} ·
                    오프셋 {result().offset}
                  </p>
                )}
              </Show>
              <Show when={produceError()}>
                <p class="notice error" role="alert">
                  {produceError()}
                </p>
              </Show>
            </form>
            <div class="tip">
              <span>↳</span>
              <p>
                먼저 <b>수신 시작</b>을 누르고 ‘수신 중’을 확인한 뒤 메시지를
                발행해 보세요.
              </p>
            </div>
          </section>
          <section class="panel consumer-panel">
            <div class="panel-heading">
              <div>
                <p class="eyebrow">02 / CONSUME</p>
                <h2>실시간 메시지</h2>
              </div>
              <span classList={{ badge: true, live: connection() === "live" }}>
                <span
                  classList={{ dot: true, green: connection() === "live" }}
                />
                {connectionLabel()}
              </span>
            </div>
            <div class="consumer-controls">
              <label class="sr-only" for="position">
                수신 시작 위치
              </label>
              <select
                id="position"
                value={position()}
                onChange={(e) => setPosition(e.currentTarget.value)}
                disabled={active()}
              >
                <option value="latest">새 메시지부터</option>
                <option value="earliest">보관된 처음부터</option>
              </select>
              <button
                class={`button ${active() ? "secondary" : "dark"}`}
                onClick={() => (active() ? stop() : start())}
              >
                {active() ? "수신 중지" : "수신 시작"}
              </button>
            </div>
            <Show when={streamError()}>
              <p class="notice error stream-error" role="alert">
                {streamError()}
              </p>
            </Show>
            <div class="stream-summary">
              <span>
                MESSAGE STREAM <b>{messages().length}</b>
              </span>
              <button
                class="text-button"
                disabled={messages().length === 0}
                onClick={() => setMessages([])}
              >
                화면 비우기
              </button>
            </div>
            <div class="message-list" aria-label="수신 메시지">
              <Show
                when={messages().length > 0}
                fallback={
                  <div class="empty">
                    <div class="empty-icon">⇄</div>
                    <h3>
                      {active()
                        ? "메시지를 기다리고 있어요"
                        : "첫 번째 메시지를 기다립니다"}
                    </h3>
                    <p>
                      수신을 시작하면 이곳에 메시지가 표시됩니다.
                      <br />
                      왼쪽에서 새로운 메시지를 보내보세요.
                    </p>
                    <span class="empty-route">
                      PRODUCER <i>→</i> KAFKA <i>→</i> YOU
                    </span>
                  </div>
                }
              >
                <For each={messages()}>
                  {(message) => (
                    <article class="message">
                      <div class="message-meta">
                        <strong>{message.key ?? "키 없음"}</strong>
                        <time>
                          {message.timestamp === null
                            ? "시간 없음"
                            : new Date(message.timestamp).toLocaleTimeString(
                                "ko-KR",
                                { hour12: false },
                              )}
                        </time>
                      </div>
                      <pre>
                        {message.payload === null
                          ? "(null · tombstone)"
                          : message.payload === ""
                            ? "(빈 메시지)"
                            : message.payload}
                      </pre>
                      <div class="message-offset">
                        <span>{message.topic}</span>
                        <span>PARTITION {message.partition}</span>
                        <span>OFFSET {message.offset}</span>
                      </div>
                    </article>
                  )}
                </For>
              </Show>
            </div>
            <div class="stream-footer">
              <span>
                <span class="dot green" /> Server-Sent Events
              </span>
              <span>최근 200개 표시 · 최신순</span>
            </div>
          </section>
        </div>
        <footer class="page-footer">
          <span>브라우저 → Rust API → Kafka → 브라우저</span>
          <span>
            발행 <b>{sent()}</b>
            <i>/</i>수신 <b>{received()}</b>
          </span>
        </footer>
      </main>
    </div>
  );
}
