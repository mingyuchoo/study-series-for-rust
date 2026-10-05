import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";

const todo = {
  id: "todo-1",
  title: "Learn SolidJS",
  description: "Reactive TODO",
  completed: false,
  created_at: "2026-10-05T00:00:00Z",
  updated_at: "2026-10-05T00:00:00Z",
};
let dispose: (() => void) | undefined;
function mount() {
  const root = document.createElement("div");
  document.body.append(root);
  dispose = render(() => <App />, root);
  return root;
}
function button(root: HTMLElement, label: string) {
  const element = [...root.querySelectorAll("button")].find(
    (button) =>
      (button.getAttribute("aria-label") ?? button.textContent?.trim()) ===
      label,
  );
  if (!element) throw new Error(`Button not found: ${label}`);
  return element;
}
function input(root: HTMLElement, selector: string, value: string) {
  const element = root.querySelector<HTMLInputElement | HTMLTextAreaElement>(
    selector,
  )!;
  element.value = value;
  element.dispatchEvent(new Event("input", { bubbles: true }));
}
function json(value: unknown, status = 200) {
  return new Response(JSON.stringify(value), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}
const rows = (root: HTMLElement) => root.querySelectorAll(".task-row");
beforeEach(() => {
  localStorage.clear();
  vi.spyOn(navigator, "language", "get").mockReturnValue("en-US");
});
afterEach(() => {
  dispose?.();
  dispose = undefined;
  document.body.replaceChildren();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  localStorage.clear();
  document.documentElement.removeAttribute("data-theme");
  document.documentElement.lang = "en";
});

describe("TODO workspace", () => {
  it("creates, cancels editing, saves, toggles, refreshes and deletes tasks", async () => {
    const fetchMock = vi.fn().mockResolvedValueOnce(json([]));
    vi.stubGlobal("fetch", fetchMock);
    const root = mount();
    expect(root.textContent).toContain("Loading your tasks");
    await vi.waitFor(() =>
      expect(root.textContent).toContain("A fresh start."),
    );
    expect(button(root, "Add task").disabled).toBe(true);
    input(root, "#new-title", "  Learn SolidJS  ");
    input(root, "#new-description", todo.description);
    fetchMock.mockResolvedValueOnce(json(todo, 201));
    button(root, "Add task").click();
    await vi.waitFor(() => expect(rows(root)).toHaveLength(1));
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "/api/todos",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({
          title: todo.title,
          description: todo.description,
        }),
      }),
    );
    expect(root.querySelector<HTMLInputElement>("#new-title")!.value).toBe("");
    button(root, `Edit ${todo.title}`).click();
    input(root, "#title-todo-1", "Discard this");
    button(root, "Cancel").click();
    expect(rows(root)[0].textContent).toContain(todo.title);
    expect(document.activeElement).toBe(button(root, `Edit ${todo.title}`));
    button(root, `Edit ${todo.title}`).click();
    input(root, "#title-todo-1", "Updated title");
    input(root, "#description-todo-1", "Updated description");
    const updated = {
      ...todo,
      title: "Updated title",
      description: "Updated description",
    };
    fetchMock.mockResolvedValueOnce(json(updated));
    button(root, "Save changes").click();
    await vi.waitFor(() =>
      expect(root.querySelector(".task-copy h3")?.textContent).toBe(
        updated.title,
      ),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      3,
      "/api/todos/todo-1",
      expect.objectContaining({
        method: "PUT",
        body: JSON.stringify({
          title: updated.title,
          description: updated.description,
        }),
      }),
    );
    fetchMock.mockResolvedValueOnce(json({ ...updated, completed: true }));
    root.querySelector<HTMLInputElement>('[type="checkbox"]')!.click();
    await vi.waitFor(() =>
      expect(root.querySelector(".is-complete")).not.toBeNull(),
    );
    expect(root.querySelector("progress")?.value).toBe(100);
    expect(fetchMock).toHaveBeenNthCalledWith(
      4,
      "/api/todos/todo-1",
      expect.objectContaining({
        method: "PUT",
        body: JSON.stringify({ completed: true }),
      }),
    );
    fetchMock.mockResolvedValueOnce(json([{ ...updated, completed: true }]));
    button(root, "Refresh tasks").click();
    await vi.waitFor(() => expect(rows(root)).toHaveLength(1));
    fetchMock.mockResolvedValueOnce(new Response(null, { status: 204 }));
    button(root, `Delete ${updated.title}`).click();
    await vi.waitFor(() =>
      expect(root.textContent).toContain("A fresh start."),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      6,
      "/api/todos/todo-1",
      expect.objectContaining({ method: "DELETE" }),
    );
    expect(root.querySelector("progress")?.value).toBe(0);
  });

  it("combines status and case-insensitive description search with accurate totals", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        json([
          todo,
          {
            ...todo,
            id: "todo-2",
            title: "Ship design",
            description: "Review UI",
            completed: true,
          },
        ]),
      ),
    );
    const root = mount();
    await vi.waitFor(() => expect(rows(root)).toHaveLength(2));
    expect(root.querySelector("progress")?.value).toBe(50);
    const filters = root.querySelectorAll<HTMLButtonElement>(".filter-button");
    filters[2].click();
    expect(rows(root)).toHaveLength(1);
    expect(rows(root)[0].textContent).toContain("Ship design");
    input(root, '[type="search"]', "REACTIVE");
    expect(root.textContent).toContain("No matching tasks.");
    filters[0].click();
    expect(rows(root)).toHaveLength(1);
    expect(rows(root)[0].textContent).toContain(todo.title);
    input(root, '[type="search"]', "unmatched");
    button(root, "Clear filters").click();
    expect(rows(root)).toHaveLength(2);
  });

  it("shows an HTTP load error, retries, and does not mistake failure for an empty list", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(json({ error: "Unavailable" }, 500))
      .mockResolvedValueOnce(json([todo]));
    vi.stubGlobal("fetch", fetchMock);
    const root = mount();
    await vi.waitFor(() =>
      expect(root.querySelector('[role="alert"]')?.textContent).toContain(
        "couldn’t be loaded",
      ),
    );
    expect(root.textContent).not.toContain("A fresh start.");
    button(root, "Refresh tasks").click();
    await vi.waitFor(() => expect(rows(root)).toHaveLength(1));
    expect(root.querySelector('[role="alert"]')).toBeNull();
  });

  it("retains draft input after a rejected save and prevents duplicate submissions", async () => {
    let rejectSave!: (reason: Error) => void;
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(json([]))
      .mockImplementationOnce(
        () =>
          new Promise<Response>((_, reject) => {
            rejectSave = reject;
          }),
      );
    vi.stubGlobal("fetch", fetchMock);
    const root = mount();
    await vi.waitFor(() =>
      expect(root.textContent).toContain("A fresh start."),
    );
    input(root, "#new-title", "Keep this draft");
    button(root, "Add task").click();
    button(root, "Saving…").click();
    expect(fetchMock).toHaveBeenCalledTimes(2);
    rejectSave(new Error("Offline"));
    await vi.waitFor(() =>
      expect(root.querySelector('[role="alert"]')?.textContent).toContain(
        "couldn’t be saved",
      ),
    );
    expect(root.querySelector<HTMLInputElement>("#new-title")?.value).toBe(
      "Keep this draft",
    );
    expect(button(root, "Add task").disabled).toBe(false);
  });

  it("preserves the edit and existing task when an update fails", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(json([todo]))
      .mockResolvedValueOnce(json({ error: "Failed" }, 500));
    vi.stubGlobal("fetch", fetchMock);
    const root = mount();
    await vi.waitFor(() => expect(rows(root)).toHaveLength(1));
    button(root, `Edit ${todo.title}`).click();
    input(root, "#title-todo-1", "   ");
    expect(button(root, "Save changes").disabled).toBe(true);
    input(root, "#title-todo-1", "My draft");
    button(root, "Save changes").click();
    await vi.waitFor(() =>
      expect(root.querySelector('[role="alert"]')).not.toBeNull(),
    );
    expect(root.querySelector<HTMLInputElement>("#title-todo-1")?.value).toBe(
      "My draft",
    );
    button(root, "Cancel").click();
    expect(root.querySelector(".task-copy h3")?.textContent).toBe(todo.title);
  });

  it("keeps the saved completion state and task when toggle or delete fails", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(json([todo]))
      .mockResolvedValueOnce(json({ error: "Failed" }, 500))
      .mockResolvedValueOnce(json({ error: "Failed" }, 500));
    vi.stubGlobal("fetch", fetchMock);
    const root = mount();
    await vi.waitFor(() => expect(rows(root)).toHaveLength(1));
    const checkbox = root.querySelector<HTMLInputElement>('[type="checkbox"]')!;
    checkbox.click();
    await vi.waitFor(() =>
      expect(root.querySelector('[role="alert"]')).not.toBeNull(),
    );
    expect(checkbox.checked).toBe(false);
    expect(root.querySelector("progress")?.value).toBe(0);
    button(root, `Delete ${todo.title}`).click();
    await vi.waitFor(() =>
      expect(root.querySelector('[role="alert"]')).not.toBeNull(),
    );
    expect(rows(root)).toHaveLength(1);
  });

  it("aborts the initial request when disposed", () => {
    const fetchMock = vi.fn<typeof fetch>(
      () => new Promise<Response>(() => {}),
    );
    vi.stubGlobal("fetch", fetchMock);
    mount();
    const signal = fetchMock.mock.calls[0][1]!.signal!;
    expect(signal.aborted).toBe(false);
    dispose!();
    dispose = undefined;
    expect(signal.aborted).toBe(true);
  });
});

function mockSystemTheme(initialDark: boolean) {
  const media = Object.assign(new EventTarget(), { matches: initialDark });
  vi.stubGlobal(
    "matchMedia",
    vi.fn(() => media),
  );
  return {
    media,
    change(dark: boolean) {
      media.matches = dark;
      media.dispatchEvent(
        Object.assign(new Event("change"), { matches: dark }),
      );
    },
  };
}

describe("language and theme preferences", () => {
  it("translates the UI, dates and accessible labels while preserving tasks, filters and drafts", async () => {
    const fetchMock = vi.fn().mockResolvedValue(json([todo]));
    vi.stubGlobal("fetch", fetchMock);
    const root = mount();
    await vi.waitFor(() => expect(rows(root)).toHaveLength(1));
    input(root, "#new-title", "내가 작성한 제목");
    input(root, '[type="search"]', "Reactive");
    root.querySelectorAll<HTMLButtonElement>(".filter-button")[1].click();
    button(root, "한국어").click();
    expect(document.documentElement.lang).toBe("ko");
    expect(document.title).toBe("todo. — 나만의 작업 공간");
    expect(root.textContent).toContain("할 일 관리, 더 간편하게.");
    expect(root.textContent).toContain("전체 1개 중 1개");
    expect(root.querySelector("time")?.textContent).toBe(
      `추가일 ${new Date(todo.created_at).toLocaleDateString("ko-KR", { month: "short", day: "numeric", year: "numeric" })}`,
    );
    expect(button(root, "한국어").getAttribute("aria-pressed")).toBe("true");
    expect(button(root, "English").getAttribute("aria-pressed")).toBe("false");
    expect(root.querySelector<HTMLInputElement>("#new-title")?.value).toBe(
      "내가 작성한 제목",
    );
    expect(root.querySelector<HTMLInputElement>('[type="search"]')?.value).toBe(
      "Reactive",
    );
    expect(
      root.querySelector('.filter-button[aria-pressed="true"]')?.textContent,
    ).toContain("진행 중");
    expect(
      root.querySelector('[type="checkbox"]')?.getAttribute("aria-label"),
    ).toBe(`${todo.title}: 완료로 변경`);
    button(root, `${todo.title} 수정`).click();
    input(root, "#title-todo-1", "수정 중인 제목");
    button(root, "English").click();
    expect(button(root, "Save changes").disabled).toBe(false);
    expect(root.querySelector<HTMLInputElement>("#title-todo-1")?.value).toBe(
      "수정 중인 제목",
    );
    button(root, "Cancel").click();
    expect(rows(root)[0].textContent).toContain(todo.title);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("translates an existing error and success notice when language changes", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(json({}, 500))
      .mockResolvedValueOnce(json([]))
      .mockResolvedValueOnce(json(todo, 201));
    vi.stubGlobal("fetch", fetchMock);
    const root = mount();
    await vi.waitFor(() =>
      expect(root.querySelector('[role="alert"]')).not.toBeNull(),
    );
    button(root, "한국어").click();
    expect(root.querySelector('[role="alert"]')?.textContent).toContain(
      "할 일을 불러오지 못했습니다.",
    );
    button(root, "할 일 새로고침").click();
    await vi.waitFor(() =>
      expect(root.textContent).toContain("새롭게 시작해 보세요."),
    );
    input(root, "#new-title", todo.title);
    button(root, "할 일 추가").click();
    await vi.waitFor(() =>
      expect(root.querySelector(".status-message")?.textContent).toContain(
        "할 일을 추가했습니다.",
      ),
    );
    button(root, "English").click();
    expect(root.querySelector(".status-message")?.textContent).toContain(
      "Task added.",
    );
  });

  it("tracks system changes only in system mode and removes its listener on unmount", () => {
    const system = mockSystemTheme(true);
    const remove = vi.spyOn(system.media, "removeEventListener");
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(json([])));
    const root = mount();
    expect(button(root, "System").getAttribute("aria-pressed")).toBe("true");
    expect(document.documentElement.dataset.theme).toBe("dark");
    system.change(false);
    expect(document.documentElement.dataset.theme).toBe("light");
    button(root, "Dark").click();
    expect(document.documentElement.dataset.theme).toBe("dark");
    system.change(true);
    system.change(false);
    expect(document.documentElement.dataset.theme).toBe("dark");
    button(root, "Light").click();
    system.change(true);
    expect(document.documentElement.dataset.theme).toBe("light");
    button(root, "System").click();
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(
      root.querySelectorAll(
        '.preference-group[aria-label="Theme"] [aria-pressed="true"]',
      ),
    ).toHaveLength(1);
    dispose!();
    dispose = undefined;
    expect(remove).toHaveBeenCalledWith("change", expect.any(Function));
  });

  it("restores the chosen language and theme after remounting", () => {
    mockSystemTheme(false);
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(json([])));
    const root = mount();
    button(root, "한국어").click();
    button(root, "다크").click();
    expect(localStorage.getItem("todo.language")).toBe("ko");
    expect(localStorage.getItem("todo.theme")).toBe("dark");
    dispose!();
    root.remove();
    const restored = mount();
    expect(button(restored, "한국어").getAttribute("aria-pressed")).toBe(
      "true",
    );
    expect(button(restored, "다크").getAttribute("aria-pressed")).toBe("true");
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("falls back to browser language and system theme for invalid stored values", () => {
    vi.spyOn(navigator, "language", "get").mockReturnValue("ko-KR");
    localStorage.setItem("todo.language", "invalid");
    localStorage.setItem("todo.theme", "invalid");
    mockSystemTheme(true);
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(json([])));
    const root = mount();
    expect(button(root, "한국어").getAttribute("aria-pressed")).toBe("true");
    expect(button(root, "시스템").getAttribute("aria-pressed")).toBe("true");
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("keeps both toggles usable when browser storage is blocked", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("Blocked");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("Blocked");
    });
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(json([])));
    const root = mount();
    button(root, "한국어").click();
    button(root, "다크").click();
    expect(document.documentElement.lang).toBe("ko");
    expect(document.documentElement.dataset.theme).toBe("dark");
  });
});
