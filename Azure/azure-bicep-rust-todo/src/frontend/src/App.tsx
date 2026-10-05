import {
  createMemo,
  createSignal,
  For,
  onCleanup,
  onMount,
  Show,
} from "solid-js";
import type { JSX } from "solid-js";
import "./App.css";
import { createPreferences } from "./preferences";
import type { Message } from "./i18n";

interface Todo {
  id: string;
  title: string;
  description?: string;
  completed: boolean;
  created_at: string;
  updated_at: string;
}
type Filter = "All tasks" | "Active" | "Completed";

function Icon(props: {
  name:
    | "check"
    | "plus"
    | "arrow"
    | "search"
    | "refresh"
    | "edit"
    | "trash"
    | "list";
}) {
  const paths = {
    check: "m5 12 4 4L19 6",
    plus: "M12 5v14M5 12h14",
    arrow: "M5 12h14m-6-6 6 6-6 6",
    search: "m21 21-5-5M18 10a8 8 0 1 1-16 0 8 8 0 0 1 16 0",
    refresh: "M20 7v5h-5M4 17v-5h5M6 6a8 8 0 0 1 13 3M5 15a8 8 0 0 0 13 3",
    edit: "m14 5 5 5M4 20l5-1L20 8a2 2 0 0 0-5-5L4 14v6Z",
    trash: "M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7M14 10v7",
    list: "M9 6h11M9 12h11M9 18h11M4 6h.01M4 12h.01M4 18h.01",
  };
  return (
    <svg
      width="20"
      height="20"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="1.6"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"
    >
      <path d={paths[props.name]} />
    </svg>
  );
}

function App() {
  const { language, setLanguage, theme, setTheme, t } = createPreferences();
  const [todos, setTodos] = createSignal<Todo[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [loaded, setLoaded] = createSignal(false);
  const [pending, setPending] = createSignal(false);
  const [error, setError] = createSignal<Message | "">("");
  const [notice, setNotice] = createSignal<Message | "">("");
  const [newTodo, setNewTodo] = createSignal({ title: "", description: "" });
  const [editingTodo, setEditingTodo] = createSignal<Todo | null>(null);
  const [filter, setFilter] = createSignal<Filter>("All tasks");
  const [search, setSearch] = createSignal("");
  const controller = new AbortController();
  let titleInput!: HTMLInputElement;

  const completed = createMemo(
    () => todos().filter((todo) => todo.completed).length,
  );
  const active = createMemo(() => todos().length - completed());
  const progress = createMemo(() =>
    todos().length ? Math.round((completed() / todos().length) * 100) : 0,
  );
  const visibleTodos = createMemo(() =>
    todos().filter((todo) => {
      const matchesStatus =
        filter() === "All tasks" ||
        (filter() === "Completed" ? todo.completed : !todo.completed);
      return (
        matchesStatus &&
        `${todo.title} ${todo.description ?? ""}`
          .toLowerCase()
          .includes(search().trim().toLowerCase())
      );
    }),
  );

  const fetchTodos = async () => {
    setLoading(true);
    setError("");
    try {
      const response = await fetch("/api/todos", { signal: controller.signal });
      if (!response.ok) throw new Error("Unable to load tasks");
      const data: Todo[] = await response.json();
      if (!controller.signal.aborted) {
        setTodos(data);
        setLoaded(true);
      }
    } catch {
      if (!controller.signal.aborted)
        setError("Your tasks couldn’t be loaded. Please try refreshing.");
    } finally {
      if (!controller.signal.aborted) setLoading(false);
    }
  };

  const mutate = async (url: string, method: string, body?: object) => {
    if (pending() || loading()) return false;
    setPending(true);
    setError("");
    setNotice("");
    try {
      const response = await fetch(url, {
        method,
        signal: controller.signal,
        ...(body
          ? {
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify(body),
            }
          : {}),
      });
      if (!response.ok) throw new Error("Unable to save task");
      if (method === "DELETE") {
        setTodos((todos) =>
          todos.filter((todo) => `/api/todos/${todo.id}` !== url),
        );
      } else {
        const saved: Todo = await response.json();
        setTodos((todos) =>
          method === "POST"
            ? [saved, ...todos]
            : todos.map((todo) => (todo.id === saved.id ? saved : todo)),
        );
      }
      return true;
    } catch {
      if (!controller.signal.aborted)
        setError(
          "That change couldn’t be saved. Your input is still here — please try again.",
        );
      return false;
    } finally {
      if (!controller.signal.aborted) setPending(false);
    }
  };

  const createTodo: JSX.EventHandler<HTMLFormElement, SubmitEvent> = async (
    event,
  ) => {
    event.preventDefault();
    if (!newTodo().title.trim()) return;
    if (
      await mutate("/api/todos", "POST", {
        title: newTodo().title.trim(),
        description: newTodo().description.trim(),
      })
    ) {
      setNewTodo({ title: "", description: "" });
      setFilter("All tasks");
      setSearch("");
      setNotice("Task added. A little more clarity for your day.");
      titleInput.focus();
    }
  };
  const saveTodo: JSX.EventHandler<HTMLFormElement, SubmitEvent> = async (
    event,
  ) => {
    event.preventDefault();
    const todo = editingTodo();
    if (!todo?.title.trim()) return;
    if (
      await mutate(`/api/todos/${todo.id}`, "PUT", {
        title: todo.title.trim(),
        description: todo.description?.trim() ?? "",
      })
    ) {
      setEditingTodo(null);
      setNotice("Task updated.");
      document.getElementById(`edit-${todo.id}`)?.focus();
    }
  };
  const focusNewTask = () => {
    titleInput.focus();
    titleInput.scrollIntoView?.({ block: "center" });
  };
  const cancelEdit = () => {
    const id = editingTodo()?.id;
    setEditingTodo(null);
    document.getElementById(`edit-${id}`)?.focus();
  };
  const busy = () => pending() || loading();
  const toggleTodo = async (todo: Todo, checkbox: HTMLInputElement) => {
    // Keep the control consistent with saved data while the request is pending.
    checkbox.checked = todo.completed;
    if (
      await mutate(`/api/todos/${todo.id}`, "PUT", {
        completed: !todo.completed,
      })
    ) {
      setNotice(
        todo.completed ? "Task marked active." : "Task completed. Nicely done.",
      );
      const updatedCheckbox = document.getElementById(`complete-${todo.id}`);
      if (updatedCheckbox) updatedCheckbox.focus();
      else
        document
          .querySelector<HTMLButtonElement>(
            '.filter-button[aria-pressed="true"]',
          )
          ?.focus();
    }
  };
  onMount(() => {
    void fetchTodos();
  });
  onCleanup(() => controller.abort());

  return (
    <div class="app-shell">
      <a class="skip-link" href="#workspace">
        {t("Skip to tasks")}
      </a>
      <header class="site-header">
        <nav class="container nav-content" aria-label={t("Main navigation")}>
          <a class="wordmark" href="#top" aria-label={t("Todo home")}>
            <span class="brand-symbol">
              <Icon name="check" />
            </span>
            todo.
          </a>
          <a class="workspace-link" href="#workspace">
            {t("My workspace")}
          </a>
          <div class="preferences">
            <div
              class="preference-group"
              role="group"
              aria-label={t("Language")}
            >
              <span class="preference-label">{t("Language")}</span>
              <div class="toggle-buttons">
                <button
                  type="button"
                  lang="ko"
                  aria-pressed={language() === "ko"}
                  onClick={() => setLanguage("ko")}
                >
                  한국어
                </button>
                <button
                  type="button"
                  lang="en"
                  aria-pressed={language() === "en"}
                  onClick={() => setLanguage("en")}
                >
                  English
                </button>
              </div>
            </div>
            <div class="preference-group" role="group" aria-label={t("Theme")}>
              <span class="preference-label">{t("Theme")}</span>
              <div class="toggle-buttons">
                <button
                  type="button"
                  aria-pressed={theme() === "system"}
                  onClick={() => setTheme("system")}
                >
                  {t("System")}
                </button>
                <button
                  type="button"
                  aria-pressed={theme() === "light"}
                  onClick={() => setTheme("light")}
                >
                  {t("Light")}
                </button>
                <button
                  type="button"
                  aria-pressed={theme() === "dark"}
                  onClick={() => setTheme("dark")}
                >
                  {t("Dark")}
                </button>
              </div>
            </div>
          </div>
          <button class="button button-primary" onClick={focusNewTask}>
            <Icon name="plus" /> {t("New task")}
          </button>
        </nav>
      </header>
      <main id="top">
        <section class="container hero" aria-labelledby="hero-title">
          <div class="hero-copy">
            <span class="eyebrow">
              <span class="tiny-dot" />{" "}
              {t("A little structure. A lot of clarity.")}
            </span>
            <h1 id="hero-title">
              {t("Make room for")}
              <br />
              {t("what matters.")}
            </h1>
            <p>
              {t(
                "Big plans start with small steps. Keep your tasks in one place and move through your day with a clear mind.",
              )}
            </p>
            <a class="text-link" href="#workspace">
              {t("Let’s get things done")} <Icon name="arrow" />
            </a>
          </div>
          <aside class="overview-card" aria-labelledby="overview-title">
            <div class="section-heading">
              <span class="icon-tile">
                <Icon name="list" />
              </span>
              <span class="badge">{t("Your workspace")}</span>
            </div>
            <h2 id="overview-title">{t("A clearer picture.")}</h2>
            <p>{t("Every small step adds up.")}</p>
            <dl class="stats">
              <div>
                <dt>{t("Total tasks")}</dt>
                <dd>{loaded() ? todos().length : "—"}</dd>
              </div>
              <div>
                <dt>{t("Active")}</dt>
                <dd>{loaded() ? active() : "—"}</dd>
              </div>
              <div>
                <dt>{t("Completed")}</dt>
                <dd>{loaded() ? completed() : "—"}</dd>
              </div>
            </dl>
            <div class="progress-label">
              <span>{t("Overall progress")}</span>
              <span>{loaded() ? `${progress()}%` : "—"}</span>
            </div>
            <progress
              max="100"
              value={progress()}
              aria-label={t("Task completion")}
            />
          </aside>
        </section>
        <section
          class="workspace-band"
          id="workspace"
          aria-labelledby="workspace-title"
        >
          <div class="container">
            <div class="workspace-heading">
              <div>
                <span class="eyebrow">{t("ONE THING AT A TIME")}</span>
                <h2 id="workspace-title">{t("Your tasks, simplified.")}</h2>
              </div>
              <p>{t("A place for everything on your mind.")}</p>
            </div>
            <div class="feedback-area">
              <Show when={error()}>
                <div class="error-message" role="alert">
                  {error() && t(error() as Message)}
                  <Show when={!pending() && !editingTodo()}>
                    <button
                      class="button button-secondary"
                      disabled={loading()}
                      onClick={() => void fetchTodos()}
                    >
                      {t("Refresh tasks")}
                    </button>
                  </Show>
                </div>
              </Show>
              <p class="status-message" role="status">
                {notice() && t(notice() as Message)}
              </p>
            </div>
            <div class="workspace-grid">
              <aside class="composer-card">
                <span class="icon-tile">
                  <Icon name="plus" />
                </span>
                <h2>{t("Add a new task")}</h2>
                <p>{t("Get it out of your head and onto your list.")}</p>
                <form onSubmit={createTodo}>
                  <label for="new-title">{t("Task title")}</label>
                  <input
                    ref={(element) => {
                      titleInput = element;
                    }}
                    id="new-title"
                    placeholder={t("What needs to get done?")}
                    required
                    value={newTodo().title}
                    disabled={pending()}
                    onInput={(event) =>
                      setNewTodo({
                        ...newTodo(),
                        title: event.currentTarget.value,
                      })
                    }
                  />
                  <label for="new-description">
                    {t("Description")} <span>{t("(optional)")}</span>
                  </label>
                  <textarea
                    id="new-description"
                    placeholder={t("Add a little more detail…")}
                    rows="4"
                    value={newTodo().description}
                    disabled={pending()}
                    onInput={(event) =>
                      setNewTodo({
                        ...newTodo(),
                        description: event.currentTarget.value,
                      })
                    }
                  />
                  <button
                    class="button button-primary full-width"
                    type="submit"
                    disabled={
                      busy() ||
                      !!editingTodo() ||
                      !loaded() ||
                      !newTodo().title.trim()
                    }
                  >
                    <Icon name="plus" />
                    {pending() ? t("Saving…") : t("Add task")}
                  </button>
                </form>
                <div class="composer-note">
                  <Icon name="check" />
                  <span>{t("One small step is a great start.")}</span>
                </div>
              </aside>
              <div class="task-panel">
                <div class="task-toolbar">
                  <div
                    class="filter-group"
                    role="group"
                    aria-label={t("Filter tasks")}
                  >
                    <For
                      each={["All tasks", "Active", "Completed"] as Filter[]}
                    >
                      {(item) => (
                        <button
                          class="filter-button"
                          aria-pressed={filter() === item}
                          disabled={!!editingTodo()}
                          onClick={() => setFilter(item)}
                        >
                          {t(item)}
                          <span>
                            {item === "All tasks"
                              ? todos().length
                              : item === "Active"
                                ? active()
                                : completed()}
                          </span>
                        </button>
                      )}
                    </For>
                  </div>
                  <button
                    class="icon-button"
                    aria-label={t("Refresh tasks")}
                    title={t("Refresh tasks")}
                    disabled={busy() || !!editingTodo()}
                    onClick={() => void fetchTodos()}
                  >
                    <Icon name="refresh" />
                  </button>
                </div>
                <div class="search-field">
                  <Icon name="search" />
                  <input
                    type="search"
                    aria-label={t("Search tasks")}
                    placeholder={t("Search your tasks…")}
                    value={search()}
                    disabled={!!editingTodo()}
                    onInput={(event) => setSearch(event.currentTarget.value)}
                  />
                </div>
                <div class="task-list" aria-busy={loading()}>
                  <Show
                    when={!loading()}
                    fallback={
                      <div class="empty-state" role="status">
                        <span class="icon-tile">
                          <Icon name="list" />
                        </span>
                        <h3>{t("Loading your tasks…")}</h3>
                        <p>{t("Making a little room for clarity.")}</p>
                      </div>
                    }
                  >
                    <Show
                      when={loaded()}
                      fallback={
                        <div class="empty-state">
                          <span class="icon-tile">
                            <Icon name="refresh" />
                          </span>
                          <h3>{t("Let’s try that again.")}</h3>
                          <p>
                            {t(
                              "Refresh your tasks to reconnect to your workspace.",
                            )}
                          </p>
                        </div>
                      }
                    >
                      <Show
                        when={visibleTodos().length > 0}
                        fallback={
                          <div class="empty-state">
                            <span class="icon-tile">
                              <Icon
                                name={
                                  filter() === "Active" && todos().length > 0
                                    ? "check"
                                    : "list"
                                }
                              />
                            </span>
                            <h3>
                              {search().trim()
                                ? t("No matching tasks.")
                                : todos().length === 0
                                  ? t("A fresh start.")
                                  : filter() === "Active"
                                    ? t("All caught up.")
                                    : t("Small steps start here.")}
                            </h3>
                            <p>
                              {search().trim()
                                ? t(
                                    "Try a different search or clear your filters.",
                                  )
                                : todos().length === 0
                                  ? t(
                                      "Add your first task and make space for what matters.",
                                    )
                                  : filter() === "Active"
                                    ? t(
                                        "No active tasks. Take a moment to enjoy it.",
                                      )
                                    : t("Completed tasks will appear here.")}
                            </p>
                            <Show when={search().trim()}>
                              <button
                                class="button button-secondary"
                                onClick={() => {
                                  setSearch("");
                                  setFilter("All tasks");
                                }}
                              >
                                {t("Clear filters")}
                              </button>
                            </Show>
                          </div>
                        }
                      >
                        <ul>
                          <For each={visibleTodos()}>
                            {(todo) => (
                              <li
                                class="task-row"
                                classList={{ "is-complete": todo.completed }}
                              >
                                <Show
                                  when={editingTodo()?.id === todo.id}
                                  fallback={
                                    <>
                                      <input
                                        id={`complete-${todo.id}`}
                                        class="task-checkbox"
                                        type="checkbox"
                                        checked={todo.completed}
                                        disabled={busy() || !!editingTodo()}
                                        aria-label={t(
                                          todo.completed
                                            ? "Mark {title} incomplete"
                                            : "Mark {title} complete",
                                          { title: todo.title },
                                        )}
                                        onChange={(event) =>
                                          void toggleTodo(
                                            todo,
                                            event.currentTarget,
                                          )
                                        }
                                      />
                                      <div class="task-copy">
                                        <h3>{todo.title}</h3>
                                        <Show when={todo.description}>
                                          <p>{todo.description}</p>
                                        </Show>
                                        <div class="task-meta">
                                          <span class="task-status">
                                            {todo.completed
                                              ? t("Completed")
                                              : t("Active")}
                                          </span>
                                          <span aria-hidden="true">·</span>
                                          <time dateTime={todo.created_at}>
                                            {t("Added {date}", {
                                              date: new Date(
                                                todo.created_at,
                                              ).toLocaleDateString(
                                                language() === "ko"
                                                  ? "ko-KR"
                                                  : "en-US",
                                                {
                                                  month: "short",
                                                  day: "numeric",
                                                  year: "numeric",
                                                },
                                              ),
                                            })}
                                          </time>
                                        </div>
                                      </div>
                                      <div class="task-actions">
                                        <button
                                          id={`edit-${todo.id}`}
                                          class="icon-button"
                                          aria-label={t("Edit {title}", {
                                            title: todo.title,
                                          })}
                                          title={t("Edit task")}
                                          disabled={busy() || !!editingTodo()}
                                          onClick={() => {
                                            setEditingTodo({ ...todo });
                                            document
                                              .getElementById(
                                                `title-${todo.id}`,
                                              )
                                              ?.focus();
                                          }}
                                        >
                                          <Icon name="edit" />
                                        </button>
                                        <button
                                          class="icon-button"
                                          aria-label={t("Delete {title}", {
                                            title: todo.title,
                                          })}
                                          title={t("Delete task")}
                                          disabled={busy() || !!editingTodo()}
                                          onClick={async () => {
                                            if (
                                              await mutate(
                                                `/api/todos/${todo.id}`,
                                                "DELETE",
                                              )
                                            ) {
                                              setNotice("Task deleted.");
                                              document
                                                .querySelector<HTMLButtonElement>(
                                                  '.filter-button[aria-pressed="true"]',
                                                )
                                                ?.focus();
                                            }
                                          }}
                                        >
                                          <Icon name="trash" />
                                        </button>
                                      </div>
                                    </>
                                  }
                                >
                                  <form
                                    class="edit-form"
                                    onSubmit={saveTodo}
                                    onKeyDown={(event) => {
                                      if (event.key === "Escape" && !pending())
                                        cancelEdit();
                                    }}
                                  >
                                    <label for={`title-${todo.id}`}>
                                      {t("Task title")}
                                    </label>
                                    <input
                                      id={`title-${todo.id}`}
                                      required
                                      value={editingTodo()!.title}
                                      disabled={pending()}
                                      onInput={(event) =>
                                        setEditingTodo({
                                          ...editingTodo()!,
                                          title: event.currentTarget.value,
                                        })
                                      }
                                    />
                                    <label for={`description-${todo.id}`}>
                                      {t("Description")}{" "}
                                      <span>{t("(optional)")}</span>
                                    </label>
                                    <textarea
                                      id={`description-${todo.id}`}
                                      rows="3"
                                      value={editingTodo()!.description ?? ""}
                                      disabled={pending()}
                                      onInput={(event) =>
                                        setEditingTodo({
                                          ...editingTodo()!,
                                          description:
                                            event.currentTarget.value,
                                        })
                                      }
                                    />
                                    <div class="edit-actions">
                                      <button
                                        type="submit"
                                        class="button button-primary"
                                        disabled={
                                          busy() || !editingTodo()!.title.trim()
                                        }
                                      >
                                        {pending()
                                          ? t("Saving…")
                                          : t("Save changes")}
                                      </button>
                                      <button
                                        type="button"
                                        class="button button-secondary"
                                        disabled={pending()}
                                        onClick={cancelEdit}
                                      >
                                        {t("Cancel")}
                                      </button>
                                    </div>
                                  </form>
                                </Show>
                              </li>
                            )}
                          </For>
                        </ul>
                      </Show>
                    </Show>
                  </Show>
                </div>
                <div class="list-footer">
                  <span>
                    {loaded()
                      ? t("{visible} of {total} tasks", {
                          visible: visibleTodos().length,
                          total: todos().length,
                        })
                      : t("Your personal task list")}
                  </span>
                  <span>{t("Less clutter. More focus.")}</span>
                </div>
              </div>
            </div>
          </div>
        </section>
        <section class="container closing-note">
          <span class="icon-tile">
            <Icon name="check" />
          </span>
          <div>
            <h2>{t("A little progress, every day.")}</h2>
            <p>
              {t(
                "You don’t have to do it all at once. Just take the next step.",
              )}
            </p>
          </div>
          <button class="button button-secondary" onClick={focusNewTask}>
            {t("Take the next step")} <Icon name="arrow" />
          </button>
        </section>
      </main>
      <footer class="site-footer">
        <div class="container footer-content">
          <div>
            <a class="wordmark" href="#top">
              todo.
            </a>
            <p>{t("A simpler space to get things done.")}</p>
          </div>
          <a href="#workspace">
            {t("Back to your tasks")} <Icon name="arrow" />
          </a>
        </div>
        <div class="container footer-bottom">
          <span>{t("Make time for what matters.")}</span>
          <span>{t("Your personal workspace")}</span>
        </div>
      </footer>
    </div>
  );
}
export default App;
