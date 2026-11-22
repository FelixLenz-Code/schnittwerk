import { listen } from "@tauri-apps/api/event";
import { api, inApp } from "./api";
import { editor } from "./editor.svelte";
import type { MatKind, PathInput, PathObj, PlotterStatus } from "./types";

type Phase = "idle" | "cutting" | "paused" | "cancelling";

/** Maps a path object's local coordinates onto the mat (mm). */
export function pathMatrix(o: PathObj, mirrorWidth: number | null, shift?: { dx: number; dy: number }): PathInput["matrix"] {
  const [bx, by, bw, bh] = o.bbox;
  let m = new DOMMatrix();
  if (shift) m = m.translate(shift.dx, shift.dy);
  if (mirrorWidth !== null) m = m.translate(mirrorWidth, 0).scale(-1, 1);
  m = m
    .translate(o.x, o.y)
    .translate(o.w / 2, o.h / 2)
    .rotate(o.rot)
    .scale(o.flipX ? -1 : 1, o.flipY ? -1 : 1)
    .translate(-o.w / 2, -o.h / 2)
    .scale(o.w / (bw || 1), o.h / (bh || 1))
    .translate(-bx, -by);
  return [m.a, m.b, m.c, m.d, m.e, m.f];
}

export function matKindFor(name: string): MatKind {
  if (name.startsWith("Portrait 3")) return "portrait8x12";
  return "none";
}

const CALIB_KEY = "schnittwerk.calibration";

function loadCalib(): { dx: number; dy: number } {
  try {
    const c = JSON.parse(localStorage.getItem(CALIB_KEY) ?? "null");
    if (c && Number.isFinite(c.dx) && Number.isFinite(c.dy)) return c;
  } catch {}
  return { dx: 0, dy: 0 };
}

class PlotterStore {
  /** Print & cut fine adjustment (mm), added to every cut coordinate. */
  calib = $state(loadCalib());

  saveCalib() {
    try {
      localStorage.setItem(CALIB_KEY, JSON.stringify(this.calib));
    } catch {}
  }

  status = $state<PlotterStatus>({ connected: false, model: null, state: "disconnected", message: null, widthMm: null });
  phase = $state<Phase>("idle");
  progress = $state({ done: 0, total: 0 });
  dialogOpen = $state(false);
  printOpen = $state(false);
  /** Searching registration marks before the cut. */
  registering = $state(false);
  /** The running job only tests the registration marks. */
  regtest = $state(false);
  lastResult = $state<{ status: string; message: string | null } | null>(null);

  #timer: ReturnType<typeof setInterval> | null = null;

  start() {
    if (!inApp || this.#timer) return;
    this.poll();
    this.#timer = setInterval(() => this.poll(), 2000);
    listen<string>("cut-phase", (e) => {
      this.registering = e.payload === "registering";
    });
    listen<{ done: number; total: number }>("cut-progress", (e) => {
      this.progress = e.payload;
    });
    listen<{ status: string; message: string | null }>("cut-finished", (e) => {
      this.phase = "idle";
      this.registering = false;
      this.lastResult = e.payload;
      if (e.payload.status === "done") editor.toast(this.regtest ? "Passermarken gefunden ✓" : "Schnitt fertig", "ok");
      else if (e.payload.status === "cancelled") editor.toast("Schnitt abgebrochen");
      else editor.toast(e.payload.message ?? "Fehler beim Schneiden", "error");
      this.poll();
    });
  }

  async poll() {
    try {
      this.status = await api.plotterStatus();
    } catch {
      /* not in app */
    }
  }

  get busy() {
    return this.phase !== "idle";
  }

  async pause(paused: boolean) {
    await api.cutPause(paused);
    this.phase = paused ? "paused" : "cutting";
  }

  async cancel() {
    this.phase = "cancelling";
    await api.cutCancel();
  }

  begin() {
    this.phase = "cutting";
    this.progress = { done: 0, total: 0 };
    this.lastResult = null;
    this.regtest = false;
    this.status = { ...this.status, state: "cutting" };
  }
}

export const plotter = new PlotterStore();
