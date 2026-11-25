// Actions shared by the properties panel and the context menu.

import { api } from "./api";
import { editor } from "./editor.svelte";
import { t } from "./i18n.svelte";
import { pathMatrix } from "./plotter.svelte";
import { regenerate, worldPathD } from "./text.svelte";
import { aabb, center, rotate } from "./geometry";
import type { Obj, PathObj } from "./types";

export const CUT_TYPES = [
  { color: "#e5484d", label: "Anschneiden", hint: "Kiss-Cut: nur die obere Schicht (Sticker bleiben auf dem Trägerpapier)" },
  { color: "#3e63dd", label: "Durchschneiden", hint: "Komplett durch – z. B. Bogenrahmen oder einzelne Sticker" },
  { color: "#30a46c", label: "Zeichnen", hint: "Mit dem Stift zeichnen" },
];

export function selectedPaths(): PathObj[] {
  return editor.selected.filter((o): o is PathObj => o.type === "path");
}

export function flip(axis: "x" | "y") {
  if (!editor.selected.length) return;
  editor.checkpoint(axis === "x" ? "Spiegeln" : "Kippen");
  for (const o of editor.selected) {
    if (axis === "x") o.flipX = !o.flipX;
    else o.flipY = !o.flipY;
  }
}

export function setCutColor(color: string) {
  const paths = selectedPaths();
  if (!paths.length) return;
  editor.checkpoint("Schnittart ändern");
  for (const o of paths) o.color = color;
}

/** Exactly one text and one other path selected. */
export function textOnPathPair(): { t: PathObj; g: PathObj } | null {
  if (editor.selected.length !== 2) return null;
  const tx = editor.selected.find((o): o is PathObj => o.type === "path" && !!o.text);
  const g = editor.selected.find((o): o is PathObj => o.type === "path" && !o.text);
  return tx && g ? { t: tx, g } : null;
}

export function putTextOnPath() {
  const p = textOnPathPair();
  if (!p || !p.t.text) return;
  editor.checkpoint("Text auf Pfad");
  p.t.text.path = worldPathD(p.g);
  p.t.text.curve = { kind: "custom" };
  p.t.text.pathOffset = 0;
  p.t.rot = 0;
  p.t.flipX = p.t.flipY = false;
  regenerate(p.t.id, "exact");
  editor.selection = [p.t.id];
  editor.toast(t("Text liegt auf dem Pfad – im Text-Bereich verschieben oder Ausrichtung ändern."), "ok");
}

export const BOOL_OPS = [
  { op: "union", label: "Vereinen", hint: "Alles zu einer Kontur verschmelzen (Verschweißen)" },
  { op: "difference", label: "Abziehen", hint: "Obere Pfade aus dem untersten ausschneiden" },
  { op: "intersect", label: "Schnittmenge", hint: "Nur die gemeinsame Fläche behalten" },
  { op: "xor", label: "Ausschließen", hint: "Überlappungen entfernen" },
] as const;

/** Boolean operation of the selected paths (bottom-most path = base). */
export async function combine(op: (typeof BOOL_OPS)[number]["op"]) {
  const ordered = editor.objects.filter((o): o is PathObj => o.type === "path" && editor.selection.includes(o.id));
  if (ordered.length < 2) return;
  try {
    const r = await api.booleanPaths(ordered.map((o) => ({ d: o.d, matrix: pathMatrix(o, null) })), op);
    const base = ordered[0];
    const [x, y, w, h] = r.bbox;
    const obj: PathObj = {
      id: crypto.randomUUID().slice(0, 8), type: "path", name: `${base.name} · ${t(BOOL_OPS.find((b) => b.op === op)!.label)}`,
      d: r.d, bbox: r.bbox, color: base.color, fill: base.fill, printFill: base.printFill, cut: base.cut,
      x, y, w, h, rot: 0, flipX: false, flipY: false, visible: true, locked: false,
    };
    editor.checkpoint("Pfade kombinieren");
    const ids = new Set(ordered.map((o) => o.id));
    const at = editor.objects.findIndex((o) => ids.has(o.id));
    editor.objects = editor.objects.filter((o) => !ids.has(o.id));
    editor.objects.splice(Math.min(at, editor.objects.length), 0, obj);
    editor.selection = [obj.id];
  } catch (e) {
    editor.error(e);
  }
}

// ---------- nesting ----------


/** Usable area and keep-out zones of the current mat / print page. */
export function sheetArea(): { area: [number, number, number, number]; keepouts: [number, number, number, number][] } {
  const r = editor.regmarks;
  return r
    ? { area: [r.originX + 1, r.originY + 1, r.width - 2, r.length - 2], keepouts: editor.regZones() }
    : { area: [5, 5, editor.mat.w - 10, editor.mat.h - 10], keepouts: [] };
}

/** Nesting outline of a unit: its cut paths, else its bounding box. */
export function unitOutline(objs: Obj[]) {
  const paths = objs.filter((o): o is PathObj => o.type === "path" && o.cut !== false);
  if (paths.length) return paths.map((o) => ({ d: o.d, matrix: pathMatrix(o, null) }));
  const b = aabb(objs)!;
  return [{ d: `M${b.x0} ${b.y0}H${b.x1}V${b.y1}H${b.x0}Z`, matrix: [1, 0, 0, 1, 0, 0] as [number, number, number, number, number, number] }];
}

export function placeUnit(objs: Obj[], rot: number, c: [number, number], dx: number, dy: number) {
  for (const o of objs) {
    const oc = center(o);
    const r = rotate({ x: oc.x - c[0], y: oc.y - c[1] }, rot);
    o.rot = (((o.rot + rot) % 360) + 360) % 360;
    o.x = c[0] + r.x + dx - o.w / 2;
    o.y = c[1] + r.y + dy - o.h / 2;
  }
}

/** Arranges the units compactly on the sheet (no undo step – callers make one).
 *  Returns how many were placed and how many did not fit. */
export async function arrangeUnits(units: Obj[][], gap: number): Promise<{ placed: number; unplaced: number }> {
  const { area, keepouts } = sheetArea();
  const r = await api.nest({ items: units.map((u) => ({ paths: unitOutline(u) })), area, keepouts, gap, rotate: true, fill: false });
  for (const p of r.placements) placeUnit(units[p.item], p.rot, p.center, p.dx, p.dy);
  // What does not fit is lined up right of the mat instead of overlapping.
  let y = area[1];
  const x = editor.mat.w + 10;
  for (const i of r.unplaced) {
    const b = aabb(units[i]);
    if (!b) continue;
    for (const o of units[i]) {
      o.x += x - b.x0;
      o.y += y - b.y0;
    }
    y += b.y1 - b.y0 + gap;
  }
  return { placed: r.placements.length, unplaced: r.unplaced.length };
}
