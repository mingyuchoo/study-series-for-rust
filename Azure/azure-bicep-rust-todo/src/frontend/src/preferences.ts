import { createEffect, createSignal, onCleanup } from "solid-js";
import { translate } from "./i18n";
import type { Language, Message } from "./i18n";

export type Theme = "system" | "light" | "dark";

function readPreference(key: string) {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function savePreference(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Settings still work for this session when storage is unavailable.
  }
}

export function createPreferences() {
  const savedLanguage = readPreference("todo.language");
  const savedTheme = readPreference("todo.theme");
  const [language, setLanguage] = createSignal<Language>(
    savedLanguage === "ko" || savedLanguage === "en"
      ? savedLanguage
      : navigator.language.toLowerCase().startsWith("ko")
        ? "ko"
        : "en",
  );
  const [theme, setTheme] = createSignal<Theme>(
    savedTheme === "light" || savedTheme === "dark" ? savedTheme : "system",
  );
  const media = window.matchMedia?.("(prefers-color-scheme: dark)");
  const [systemDark, setSystemDark] = createSignal(media?.matches ?? false);
  const updateSystemTheme = (event: MediaQueryListEvent) =>
    setSystemDark(event.matches);
  media?.addEventListener("change", updateSystemTheme);
  onCleanup(() => media?.removeEventListener("change", updateSystemTheme));

  const t = (message: Message, values?: Record<string, string | number>) =>
    translate(language(), message, values);

  createEffect(() => {
    document.documentElement.lang = language();
    document.title = t("todo. — Your personal workspace");
    document
      .querySelector('meta[name="description"]')
      ?.setAttribute(
        "content",
        t(
          "A simpler space to organize your tasks, clear your mind, and make room for what matters.",
        ),
      );
    savePreference("todo.language", language());
  });

  createEffect(() => {
    const selected = theme();
    const resolved =
      selected === "system" ? (systemDark() ? "dark" : "light") : selected;
    document.documentElement.dataset.theme = resolved;
    document
      .querySelector('meta[name="theme-color"]')
      ?.setAttribute("content", resolved === "dark" ? "#151719" : "#ffffff");
    savePreference("todo.theme", selected);
  });

  return { language, setLanguage, theme, setTheme, t };
}
