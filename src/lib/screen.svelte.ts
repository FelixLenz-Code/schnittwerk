// Real size on screen, per monitor. Every monitor (name + resolution +
// scale) keeps its own calibration; when the window moves to another
// monitor the matching value is used, and an uncalibrated monitor is
// pointed out once the feature is in use.

import { inApp } from "./api";
import { DEFAULT_PX_PER_MM, editor } from "./editor.svelte";

const KEY = "schnittwerk.screenCalib";
/** Single value of version 0.8.3 (before calibration was per monitor). */
const OLD_KEY = "schnittwerk.screenPxPerMm";

type Calibrations = Record<string, number>;

function load(): Calibrations {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    return v && typeof v === "object" ? v : {};
  } catch {
    return {};
  }
}

function store(c: Calibrations) {
  try {
    localStorage.setItem(KEY, JSON.stringify(c));
  } catch {}
}

export const screen = $state({
  /** Current monitor: key and readable label ("DP-8 · 1920 × 1080"). */
  key: "",
  label: "",
  calibrated: false,
  /** Hint at the zoom bar: first use on this monitor, or another monitor. */
  hint: null as null | "first" | "switch",
  dialogOpen: false,
});

let calibs: Calibrations = load();
/** Monitors whose hint was closed with "Later" (this session). */
const dismissed = new Set<string>();

function apply() {
  const v = calibs[screen.key];
  screen.calibrated = typeof v === "number" && v > 0;
  setPxPerMm(screen.calibrated ? v : DEFAULT_PX_PER_MM);
}

/** New scale; the zoom percentage stays, so 1:1 remains real size and
 *  a design keeps its physical size when moving to another monitor. */
function setPxPerMm(px: number) {
  if (Math.abs(px - editor.screenPxPerMm) < 1e-6) return;
  const pct = editor.zoomPercent;
  editor.screenPxPerMm = px;
  editor.setZoomPercent(pct);
}

async function detect() {
  if (!inApp) return;
  try {
    const { currentMonitor } = await import("@tauri-apps/api/window");
    const m = await currentMonitor();
    if (!m) return;
    const key = `${m.name ?? "?"}|${m.size.width}x${m.size.height}|${m.scaleFactor}`;
    if (key === screen.key) return;
    const first = !screen.key;
    screen.key = key;
    screen.label = `${m.name ?? "Bildschirm"} · ${m.size.width} × ${m.size.height}`;
    // Take over the single value of 0.8.3 for the monitor it was made on.
    const old = parseFloat(localStorage.getItem(OLD_KEY) ?? "");
    if (Number.isFinite(old) && old > 0 && !Object.keys(calibs).length) {
      calibs[key] = old;
      store(calibs);
      localStorage.removeItem(OLD_KEY);
    }
    apply();
    // Another monitor that is not calibrated yet – only worth a word if
    // real size is used at all (some monitor was calibrated before).
    if (!screen.calibrated && Object.keys(calibs).length && !dismissed.has(key)) screen.hint = first ? "first" : "switch";
    else if (screen.calibrated) screen.hint = null;
  } catch {
    /* no monitor information (browser preview, other platforms) */
  }
}

/** Watches which monitor the window is on. */
export async function startScreenWatch() {
  await detect();
  if (!inApp) return;
  let timer = 0;
  const later = () => {
    clearTimeout(timer);
    timer = window.setTimeout(detect, 400);
  };
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const w = getCurrentWindow();
    await w.onMoved(later);
    await w.onScaleChanged(later);
  } catch {}
  // Fallback: some window managers move windows without telling.
  setInterval(detect, 5000);
}

/** Uses real size (1:1 button, typed zoom): hint once if not calibrated. */
export function noteRealSizeUse() {
  if (!screen.calibrated && screen.key && !dismissed.has(screen.key)) screen.hint = "first";
}

export function dismissHint() {
  if (screen.key) dismissed.add(screen.key);
  screen.hint = null;
}

export function saveCalibration(pxPerMm: number) {
  if (screen.key) {
    calibs[screen.key] = pxPerMm;
    store(calibs);
  }
  apply();
  // Without monitor information the value still applies to this session.
  if (!screen.key) setPxPerMm(pxPerMm);
  screen.hint = null;
}

export function resetCalibration() {
  if (screen.key) {
    delete calibs[screen.key];
    store(calibs);
  }
  apply();
}

export function openCalibration() {
  screen.hint = null;
  screen.dialogOpen = true;
}
