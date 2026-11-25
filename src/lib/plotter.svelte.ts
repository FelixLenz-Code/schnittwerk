import { listen } from "@tauri-apps/api/event";
import { api, inApp } from "./api";
import { editor, matKindOf, type Mat } from "./editor.svelte";
import { t } from "./i18n.svelte";
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

/** Cutting-mat setting the device gets for a work area. */
export function matKindFor(m: Mat): MatKind {
  if (m.cut) return m.cut;
  const k = matKindOf(m);
  // Large designs are cut tile by tile on the mat.
  if (k === "mat" || (k === "custom" && !m.roll)) return "portrait8x12";
  return "none";
}

const BLADE_KEY = "schnittwerk.blade";
const TIME_KEY = "schnittwerk.timeFactor";

function loadTimeFactor(): number {
  try {
    const f = parseFloat(localStorage.getItem(TIME_KEY) ?? "");
    return Number.isFinite(f) && f > 0 ? f : 1;
  } catch {
    return 1;
  }
}

/** Where an interrupted job continues (indices among the passes with lines). */
export interface ResumePoint {
  pass: number;
  stroke: number;
  strokes: number;
}

/** Cut distance of the current blade (wear counter). */
export interface BladeLife {
  /** Millimetres cut with the blade since it was changed. */
  mm: number;
  /** Number of cut jobs. */
  jobs: number;
  /** When the blade was put in (ISO date). */
  since: string;
  /** Show a reminder after this many metres. */
  limitM: number;
}

function loadBlade(): BladeLife {
  try {
    const b = JSON.parse(localStorage.getItem(BLADE_KEY) ?? "null");
    if (b && Number.isFinite(b.mm)) return { mm: b.mm, jobs: b.jobs ?? 0, since: b.since ?? new Date().toISOString(), limitM: b.limitM ?? 150 };
  } catch {}
  return { mm: 0, jobs: 0, since: new Date().toISOString(), limitM: 150 };
}

const CALIB_KEY = "schnittwerk.calibration";
const BLE_KEY = "schnittwerk.bleDevice";
/** Pause between automatic Bluetooth reconnect attempts. */
const RECONNECT_MS = 15000;

type KnownBle = { id: string; name: string };

function loadBle(): KnownBle | null {
  try {
    const d = JSON.parse(localStorage.getItem(BLE_KEY) ?? "null");
    return d && typeof d.id === "string" ? d : null;
  } catch {
    return null;
  }
}

function loadCalib(): { dx: number; dy: number } {
  try {
    const c = JSON.parse(localStorage.getItem(CALIB_KEY) ?? "null");
    if (c && Number.isFinite(c.dx) && Number.isFinite(c.dy)) return c;
  } catch {}
  return { dx: 0, dy: 0 };
}

class PlotterStore {
  /** Bluetooth cutter to reconnect to automatically (last successful connection). */
  knownBle = $state<KnownBle | null>(loadBle());
  /** An automatic Bluetooth reconnect is in progress. */
  reconnecting = $state(false);
  #lastAttempt = 0;

  rememberBle(d: KnownBle) {
    this.knownBle = d;
    try {
      localStorage.setItem(BLE_KEY, JSON.stringify(d));
    } catch {}
  }

  forgetBle() {
    this.knownBle = null;
    try {
      localStorage.removeItem(BLE_KEY);
    } catch {}
  }

  /** Blade wear counter. */
  blade = $state<BladeLife>(loadBlade());
  /** Blade cut length (mm) of the running job, counted when it ends. */
  #jobBladeMm = 0;

  saveBlade() {
    try {
      localStorage.setItem(BLADE_KEY, JSON.stringify(this.blade));
    } catch {}
  }

  /** A new blade was put in. */
  resetBlade() {
    this.blade = { mm: 0, jobs: 0, since: new Date().toISOString(), limitM: this.blade.limitM };
    this.saveBlade();
  }

  get bladeWorn() {
    return this.blade.mm / 1000 >= this.blade.limitM;
  }

  /** Print & cut fine adjustment (mm), added to every cut coordinate. */
  calib = $state(loadCalib());

  saveCalib() {
    try {
      localStorage.setItem(CALIB_KEY, JSON.stringify(this.calib));
    } catch {}
  }

  status = $state<PlotterStatus>({ connected: false, model: null, connection: null, state: "disconnected", message: null, widthMm: null });
  phase = $state<Phase>("idle");
  progress = $state({ done: 0, total: 0 });
  dialogOpen = $state(false);
  printOpen = $state(false);
  /** Searching registration marks before the cut. */
  registering = $state(false);
  /** The running job only tests the registration marks. */
  regtest = $state(false);
  /** Index of the pass currently being cut (multi-pass jobs). */
  pass = $state(0);
  /** All data of the current pass is sent; the cutter works through its buffer. */
  draining = $state(false);
  /** Passes of the running job as sent (colour + material name), fixed at start. */
  job = $state<{ color: string; name: string }[]>([]);
  lastResult = $state<{ status: string; message: string | null; resume?: ResumePoint | null } | null>(null);

  /** Interrupted job that can be continued: `key` identifies its cut data. */
  resume = $state<{ key: string; point: ResumePoint } | null>(null);
  /** Key of the running job (see `resume`). */
  #jobKey: string | null = null;

  /** Measured / estimated cut time (learns from finished jobs). */
  timeFactor = $state(loadTimeFactor());
  /** Running job: start (ms), raw estimate (s), time spent paused (ms). */
  timing = $state<{ start: number; estimate: number; paused: number; pausedAt: number | null } | null>(null);

  #timer: ReturnType<typeof setInterval> | null = null;

  start() {
    if (!inApp || this.#timer) return;
    this.poll();
    this.#timer = setInterval(() => this.poll(), 2000);
    listen<string | { pass: number } | { draining: number }>("cut-phase", (e) => {
      const p = e.payload;
      this.registering = p === "registering";
      if (typeof p === "object" && "pass" in p) {
        this.pass = p.pass;
        this.draining = false;
      } else if (typeof p === "object" && "draining" in p) {
        this.pass = p.draining;
        this.draining = true;
      }
    });
    listen<{ done: number; total: number }>("cut-progress", (e) => {
      this.progress = e.payload;
    });
    listen<{ status: string; message: string | null; resume: ResumePoint | null }>("cut-finished", (e) => {
      this.#learnTime(e.payload.status === "done");
      // Interrupted after something was cut: offer to continue there.
      if (e.payload.resume && this.#jobKey && !this.regtest) {
        this.resume = { key: this.#jobKey, point: e.payload.resume };
      } else if (e.payload.status === "done") {
        this.resume = null;
      }
      this.#jobKey = null;
      // Blade wear: the whole job when done, otherwise the part sent so far.
      const share = e.payload.status === "done" ? 1 : this.progress.total ? this.progress.done / this.progress.total : 0;
      if (this.#jobBladeMm > 0 && share > 0) {
        const wasWorn = this.bladeWorn;
        this.blade.mm += this.#jobBladeMm * share;
        this.blade.jobs += 1;
        this.saveBlade();
        if (!wasWorn && this.bladeWorn) editor.toast(t("Das Messer hat {m} m geschnitten – Zeit zum Prüfen oder Wechseln.", { m: Math.round(this.blade.mm / 1000) }), "error");
      }
      this.#jobBladeMm = 0;
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
      return;
    }
    this.autoReconnect();
  }

  /** Reconnects to the remembered Bluetooth cutter while nothing is connected. */
  autoReconnect() {
    const known = this.knownBle;
    if (!known || this.status.connected || this.reconnecting || this.busy) return;
    if (Date.now() - this.#lastAttempt < RECONNECT_MS) return;
    this.#lastAttempt = Date.now();
    this.reconnecting = true;
    api
      .bleConnect(known.id)
      .then((name) => {
        editor.toast(t("{name} automatisch per Bluetooth verbunden", { name }), "ok");
        return this.poll();
      })
      .catch(() => {
        /* switched off or out of range – try again later */
      })
      .finally(() => {
        this.reconnecting = false;
      });
  }

  get busy() {
    return this.phase !== "idle";
  }

  async pause(paused: boolean) {
    await api.cutPause(paused);
    this.phase = paused ? "paused" : "cutting";
    const t = this.timing;
    if (t) {
      if (paused) t.pausedAt = Date.now();
      else if (t.pausedAt) {
        t.paused += Date.now() - t.pausedAt;
        t.pausedAt = null;
      }
    }
  }

  /** Corrects the time factor with a finished job (paused jobs too, minus the pause). */
  #learnTime(done: boolean) {
    const t = this.timing;
    this.timing = null;
    if (!done || !t || t.estimate < 20) return;
    const actual = (Date.now() - t.start - t.paused - (t.pausedAt ? Date.now() - t.pausedAt : 0)) / 1000;
    const ratio = Math.min(4, Math.max(0.25, actual / t.estimate));
    this.timeFactor = Math.min(4, Math.max(0.25, this.timeFactor * 0.6 + ratio * 0.4));
    try {
      localStorage.setItem(TIME_KEY, String(this.timeFactor));
    } catch {}
  }

  async cancel() {
    this.phase = "cancelling";
    await api.cutCancel();
  }

  /**
   * `bladeMm` = length cut with the blade (wear counter), `estimate` = raw
   * time estimate in seconds, `key` = identity of the cut data (to continue
   * after an interruption).
   */
  begin(job: { color: string; name: string }[] = [], bladeMm = 0, estimate = 0, key: string | null = null) {
    this.#jobBladeMm = bladeMm;
    this.#jobKey = key;
    this.timing = estimate > 0 ? { start: Date.now(), estimate, paused: 0, pausedAt: null } : null;
    this.job = job;
    this.phase = "cutting";
    this.progress = { done: 0, total: 0 };
    this.lastResult = null;
    this.regtest = false;
    this.pass = 0;
    this.draining = false;
    this.status = { ...this.status, state: "cutting" };
  }
}

export const plotter = new PlotterStore();
