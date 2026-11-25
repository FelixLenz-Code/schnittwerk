// Window-wide UI state shared by the menu bar, header and dialogs.

export type Theme = "system" | "light" | "dark";

function loadTheme(): Theme {
  try {
    const v = localStorage.getItem("schnittwerk.theme");
    return v === "light" || v === "dark" ? v : "system";
  } catch {
    return "system";
  }
}

export const ui = $state({
  theme: loadTheme(),
  settingsOpen: false,
  aboutOpen: false,
  /** Plotter menu in the header (also opened from the menu bar). */
  deviceMenuOpen: false,
  /** Help chapter to open in the help tab (set by the menu bar). */
  helpSection: null as string | null,
});

export function setTheme(theme: Theme) {
  ui.theme = theme;
  if (theme === "system") document.documentElement.removeAttribute("data-theme");
  else document.documentElement.setAttribute("data-theme", theme);
  try {
    localStorage.setItem("schnittwerk.theme", theme);
  } catch {}
}

export function isDark(): boolean {
  return ui.theme === "dark" || (ui.theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
}

if (typeof document !== "undefined") setTheme(ui.theme);
