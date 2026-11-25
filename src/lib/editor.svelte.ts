import { aabb, uid } from "./geometry";
import { refreshShape } from "./shapes";
import type { ImageAsset, Obj, RegMarks, TraceParams, TraceResult, Tool } from "./types";

export interface TracePreview {
  objId: string;
  result: TraceResult;
  ms: number;
}

export interface Mat {
  name: string;
  w: number;
  h: number;
  /** mat = cutting mat, page = paper size, roll = media without mat,
   *  custom = free size (large designs, cut in tiles). Missing = by name. */
  kind?: "mat" | "page" | "roll" | "custom";
  /** custom: cut from the roll (tiles across the roll width) instead of on the mat. */
  roll?: boolean;
}

export const MATS: Mat[] = [
  { name: "Portrait 3 · 8″ × 12″", w: 203.2, h: 304.8, kind: "mat" },
  { name: "A4 hoch", w: 210, h: 297, kind: "page" },
  { name: "A4 quer", w: 297, h: 210, kind: "page" },
  { name: "Letter", w: 215.9, h: 279.4, kind: "page" },
];

/** Cutting width of the Portrait 3 (roll media). */
export const ROLL_WIDTH = 203.2;
export const ROLL_MAX = 5000;

export function rollMat(length: number): Mat {
  return { name: "Rolle ohne Matte", w: ROLL_WIDTH, h: Math.min(ROLL_MAX, Math.max(50, length)), kind: "roll" };
}

export function customMat(w: number, h: number, roll = false): Mat {
  return { name: "Eigene Größe", w: Math.min(3000, Math.max(20, w)), h: Math.min(ROLL_MAX, Math.max(20, h)), kind: "custom", roll };
}

export function matKindOf(m: Mat): NonNullable<Mat["kind"]> {
  return m.kind ?? (m.name.startsWith("Portrait 3") ? "mat" : "page");
}

export interface Guides {
  /** x positions of vertical guides (mm). */
  v: number[];
  /** y positions of horizontal guides (mm). */
  h: number[];
}

function loadFlag(key: string, fallback: boolean): boolean {
  try {
    const v = localStorage.getItem("schnittwerk." + key);
    return v === null ? fallback : v === "1";
  } catch {
    return fallback;
  }
}

function loadNum(key: string, fallback: number): number {
  try {
    const v = parseFloat(localStorage.getItem("schnittwerk." + key) ?? "");
    return Number.isFinite(v) && v > 0 ? v : fallback;
  } catch {
    return fallback;
  }
}

function saveNum(key: string, v: number) {
  try {
    localStorage.setItem("schnittwerk." + key, String(v));
  } catch {}
}

function saveFlag(key: string, v: boolean) {
  try {
    localStorage.setItem("schnittwerk." + key, v ? "1" : "0");
  } catch {}
}

export const PAGES = {
  A4: { w: 210, h: 297, media: "A4" },
  Letter: { w: 215.9, h: 279.4, media: "Letter" },
} as const;
export type PageName = keyof typeof PAGES;

/** Zone around each registration mark that must stay empty (mm). */
export const REG_ZONE = 20;
export const REG_LINE_WIDTH = 0.5;

/**
 * SVG path (stroke width REG_LINE_WIDTH, miter joins) of the two corner
 * brackets. The lines are inset by half their width so the *outer* edges
 * lie exactly at origin + width / origin + length, which is how the plotter
 * measures the mark distance. The square is [originX, originY, 5, 5].
 */
export function regBracketPath(r: RegMarks): string {
  const h = REG_LINE_WIDTH / 2;
  const right = r.originX + r.width, bottom = r.originY + r.length;
  return (
    `M${right - REG_ZONE} ${r.originY + h}H${right - h}V${r.originY + REG_ZONE} ` +
    `M${r.originX + REG_ZONE} ${bottom - h}H${r.originX + h}V${bottom - REG_ZONE}`
  );
}

export const DEFAULT_TRACE: TraceParams = {
  mode: { kind: "threshold", level: null, invert: false },
  brightness: 0,
  contrast: 0,
  blur: 0,
  flatten: false,
  despeckle: 6,
  smoothness: 40,
  grow: 0,
  outerOnly: false,
  curves: true,
  maxSize: 2000,
};

type Toast = { id: number; text: string; kind: "info" | "error" | "ok" };

const HISTORY_LIMIT = 150;

class Editor {
  objects = $state<Obj[]>([]);
  images = $state<Record<string, ImageAsset>>({});
  selection = $state<string[]>([]);
  tool = $state<Tool>("select");
  view = $state({ zoom: 2.5, panX: 60, panY: 40 });
  viewport = $state({ w: 800, h: 600 });
  mat = $state<Mat>(MATS[0]);
  showGrid = $state(true);
  cursor = $state<{ x: number; y: number } | null>(null);
  rightTab = $state<"object" | "trace" | "sticker" | "layers">("object");

  /** Guide lines (dragged out of the rulers). */
  guides = $state<Guides>({ v: [], h: [] });
  /** Snap moved objects and drawn shapes to guides, mat and other objects. */
  #snap = $state(loadFlag("snap", true));
  get snap() {
    return this.#snap;
  }
  set snap(v: boolean) {
    this.#snap = v;
    saveFlag("snap", v);
  }
  /** Snap moving to the grid and rotating to angle steps (Alt while dragging = free). */
  #gridSnap = $state(loadFlag("gridSnap", true));
  get gridSnap() {
    return this.#gridSnap;
  }
  set gridSnap(v: boolean) {
    this.#gridSnap = v;
    saveFlag("gridSnap", v);
  }
  /** Grid step for moving (mm). */
  #gridStep = $state(loadNum("gridStep", 5));
  get gridStep() {
    return this.#gridStep;
  }
  set gridStep(v: number) {
    this.#gridStep = v;
    saveNum("gridStep", v);
  }
  /** Angle step for rotating (degrees). */
  #rotStep = $state(loadNum("rotStep", 15));
  get rotStep() {
    return this.#rotStep;
  }
  set rotStep(v: number) {
    this.#rotStep = v;
    saveNum("rotStep", v);
  }
  /** Size fields and on-canvas dimensions keep the aspect ratio. */
  keepRatio = $state(true);
  /** Material chosen for this project (new-project assistant), preselected when cutting. */
  material = $state<string | null>(null);
  /** Sticker assistant default: kiss-cut + through-cut backing ("die") or kiss-cut only. */
  stickerStyle = $state<"die" | "kiss">("die");
  /** Increases with every change (autosave). */
  rev = $state(0);

  /** Print & cut: the work area becomes a printable page with registration marks. */
  /** insetX/insetY: distance of the marks from the paper edge (mm). */
  printCut = $state<{ enabled: boolean; page: PageName; insetX: number; insetY: number }>({ enabled: false, page: "A4", insetX: 15, insetY: 10 });

  regmarks = $derived.by((): RegMarks | null => {
    if (!this.printCut.enabled) return null;
    const pg = PAGES[this.printCut.page];
    const { insetX: ix, insetY: iy } = this.printCut;
    return { originX: ix, originY: iy, width: pg.w - 2 * ix, length: pg.h - 2 * iy };
  });

  traceParams = $state<TraceParams>(structuredClone(DEFAULT_TRACE));
  tracePreview = $state<TracePreview | null>(null);
  tracing = $state(false);
  /** When set, a click on the trace target image samples a color. */
  picking = $state<((x: number, y: number) => void) | null>(null);

  filePath = $state<string | null>(null);
  dirty = $state(false);
  toasts = $state<Toast[]>([]);

  #undo: Obj[][] = [];
  #redo: Obj[][] = [];
  canUndo = $state(false);
  canRedo = $state(false);

  selected = $derived(this.objects.filter((o) => this.selection.includes(o.id)));

  // ---------- history ----------

  /** Call once before a change; the current state becomes an undo step. */
  checkpoint() {
    this.#undo.push($state.snapshot(this.objects) as Obj[]);
    if (this.#undo.length > HISTORY_LIMIT) this.#undo.shift();
    this.#redo = [];
    this.#syncHistory();
    this.dirty = true;
    this.rev++;
  }

  undo() {
    const prev = this.#undo.pop();
    if (!prev) return;
    this.#redo.push($state.snapshot(this.objects) as Obj[]);
    this.objects = prev;
    this.#afterHistory();
  }

  redo() {
    const next = this.#redo.pop();
    if (!next) return;
    this.#undo.push($state.snapshot(this.objects) as Obj[]);
    this.objects = next;
    this.#afterHistory();
  }

  #afterHistory() {
    const ids = new Set(this.objects.map((o) => o.id));
    this.selection = this.selection.filter((id) => ids.has(id));
    this.dirty = true;
    this.rev++;
    this.#syncHistory();
  }

  #syncHistory() {
    this.canUndo = this.#undo.length > 0;
    this.canRedo = this.#redo.length > 0;
  }

  resetHistory() {
    this.#undo = [];
    this.#redo = [];
    this.#syncHistory();
  }

  /** Empties the document (no confirmation – callers ask first). */
  resetDocument() {
    this.objects = [];
    this.images = {};
    this.selection = [];
    this.tracePreview = null;
    this.filePath = null;
    this.guides = { v: [], h: [] };
    this.material = null;
    this.dirty = false;
    this.resetHistory();
  }

  // ---------- objects ----------

  /** Sets width or height (mm), around the centre; honours `keepRatio`. */
  setSize(o: Obj, key: "w" | "h", v: number) {
    if (!(v > 0) || o.locked) return;
    this.checkpoint();
    const ratio = o.h / o.w;
    const cx = o.x + o.w / 2, cy = o.y + o.h / 2;
    if (key === "w") {
      o.w = v;
      if (this.keepRatio) o.h = v * ratio;
    } else {
      o.h = v;
      if (this.keepRatio) o.w = v / ratio;
    }
    o.x = cx - o.w / 2;
    o.y = cy - o.h / 2;
    if (o.type === "path") refreshShape(o);
  }

  byId(id: string): Obj | undefined {
    return this.objects.find((o) => o.id === id);
  }

  add(objs: Obj[], select = true) {
    this.checkpoint();
    this.objects.push(...objs);
    if (select) this.selection = objs.map((o) => o.id);
  }

  removeSelected() {
    if (!this.selection.length) return;
    this.checkpoint();
    const sel = new Set(this.selection);
    this.objects = this.objects.filter((o) => !sel.has(o.id) || o.locked);
    this.selection = [];
    if (this.tracePreview && sel.has(this.tracePreview.objId)) this.tracePreview = null;
  }

  duplicateSelected() {
    if (!this.selection.length) return;
    const copies = this.selected.map((o) => ({
      ...($state.snapshot(o) as Obj),
      id: uid(),
      name: o.name + " Kopie",
      x: o.x + 5,
      y: o.y + 5,
      locked: false,
    }));
    const remap = new Map<string, string>();
    for (const c of copies) if (c.group) c.group = remap.get(c.group) ?? remap.set(c.group, uid()).get(c.group);
    this.add(copies);
  }

  selectAll() {
    this.selection = this.objects.filter((o) => o.visible && !o.locked).map((o) => o.id);
  }

  nudge(dx: number, dy: number) {
    const movable = this.selected.filter((o) => !o.locked);
    if (!movable.length) return;
    this.checkpoint();
    for (const o of movable) {
      o.x += dx;
      o.y += dy;
    }
  }

  /** Moves selected objects up (+1) or down (-1) in the stacking order. */
  restack(dir: 1 | -1) {
    if (!this.selection.length) return;
    this.checkpoint();
    const arr = [...this.objects];
    const sel = new Set(this.selection);
    const idx = arr.map((o, i) => (sel.has(o.id) ? i : -1)).filter((i) => i >= 0);
    if (dir === 1) idx.reverse();
    for (const i of idx) {
      const j = i + dir;
      if (j < 0 || j >= arr.length || sel.has(arr[j].id)) continue;
      [arr[i], arr[j]] = [arr[j], arr[i]];
    }
    this.objects = arr;
  }

  align(mode: "left" | "hcenter" | "right" | "top" | "vcenter" | "bottom", toMat = false) {
    const objs = this.selected.filter((o) => !o.locked);
    if (!objs.length) return;
    const ref = toMat || objs.length === 1
      ? { x0: 0, y0: 0, x1: this.mat.w, y1: this.mat.h }
      : aabb(objs)!;
    this.checkpoint();
    for (const o of objs) {
      const b = aabb([o])!;
      let dx = 0, dy = 0;
      if (mode === "left") dx = ref.x0 - b.x0;
      if (mode === "right") dx = ref.x1 - b.x1;
      if (mode === "hcenter") dx = (ref.x0 + ref.x1) / 2 - (b.x0 + b.x1) / 2;
      if (mode === "top") dy = ref.y0 - b.y0;
      if (mode === "bottom") dy = ref.y1 - b.y1;
      if (mode === "vcenter") dy = (ref.y0 + ref.y1) / 2 - (b.y0 + b.y1) / 2;
      o.x += dx;
      o.y += dy;
    }
  }

  // ---------- print & cut ----------

  setPrintCut(enabled: boolean, page: PageName = this.printCut.page) {
    this.printCut.enabled = enabled;
    this.printCut.page = page;
    const pg = PAGES[page];
    this.mat = enabled ? { name: `${page} · Print & Cut`, w: pg.w, h: pg.h } : MATS[0];
    this.fitMat();
  }

  /** Rectangles [x, y, w, h] inside the mark frame that must stay empty. */
  regZones(): [number, number, number, number][] {
    const r = this.regmarks;
    if (!r) return [];
    const z = REG_ZONE;
    return [
      [r.originX, r.originY, z, z],
      [r.originX + r.width - z, r.originY, z, z],
      [r.originX, r.originY + r.length - z, z, z],
    ];
  }

  // ---------- groups ----------

  /** Adds every object that shares a group with one of `ids`. */
  expandGroups(ids: string[]): string[] {
    const groups = new Set(this.objects.filter((o) => ids.includes(o.id) && o.group).map((o) => o.group));
    if (!groups.size) return ids;
    return [...new Set([...ids, ...this.objects.filter((o) => o.group && groups.has(o.group)).map((o) => o.id)])];
  }

  groupSelected() {
    if (this.selection.length < 2) return;
    this.checkpoint();
    const g = uid();
    for (const o of this.selected) o.group = g;
  }

  ungroupSelected() {
    if (!this.selected.some((o) => o.group)) return;
    this.checkpoint();
    for (const o of this.selected) delete o.group;
  }

  // ---------- view ----------

  zoomAt(factor: number, sx: number, sy: number) {
    const z = Math.min(60, Math.max(0.2, this.view.zoom * factor));
    const f = z / this.view.zoom;
    this.view.panX = sx - (sx - this.view.panX) * f;
    this.view.panY = sy - (sy - this.view.panY) * f;
    this.view.zoom = z;
  }

  zoomBy(factor: number) {
    this.zoomAt(factor, this.viewport.w / 2, this.viewport.h / 2);
  }

  fitMat() {
    const pad = 56;
    const z = Math.min((this.viewport.w - pad * 2) / this.mat.w, (this.viewport.h - pad * 2) / this.mat.h);
    this.view.zoom = Math.max(0.2, z);
    this.view.panX = (this.viewport.w - this.mat.w * this.view.zoom) / 2;
    this.view.panY = (this.viewport.h - this.mat.h * this.view.zoom) / 2;
  }

  // ---------- messages ----------

  #toastId = 0;
  toast(text: string, kind: Toast["kind"] = "info") {
    const id = ++this.#toastId;
    this.toasts.push({ id, text, kind });
    setTimeout(() => (this.toasts = this.toasts.filter((t) => t.id !== id)), kind === "error" ? 6000 : 3000);
  }

  error(e: unknown) {
    this.toast(e instanceof Error ? e.message : String(e), "error");
  }
}

export const editor = new Editor();
