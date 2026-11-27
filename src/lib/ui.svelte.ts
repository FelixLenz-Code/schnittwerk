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
  /** Request to open (and scroll to) a collapsible panel section. */
  sectionRequest: { id: "", n: 0 },
  /** Layer list at the bottom of the side panel. */
  layersOpen: loadFlag("layersOpen", false),
  /** "Next steps" card after tracing / making stickers. */
  nextSteps: null as null | { kind: "trace" | "sticker"; sel: string },
});

function loadFlag(key: string, def: boolean): boolean {
  try {
    const v = localStorage.getItem(`schnittwerk.${key}`);
    return v === null ? def : v === "1";
  } catch {
    return def;
  }
}

export function setLayersOpen(v: boolean) {
  ui.layersOpen = v;
  try {
    localStorage.setItem("schnittwerk.layersOpen", v ? "1" : "0");
  } catch {}
}

/** Opens a collapsible section (Section.svelte) and scrolls it into view. */
export function openSection(id: string) {
  try {
    localStorage.setItem(`schnittwerk.section.${id}`, "1");
  } catch {}
  ui.sectionRequest = { id, n: ui.sectionRequest.n + 1 };
}

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
