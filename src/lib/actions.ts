// Actions shared by the side panel, the menu bar and the context menu.

import { api } from "./api";
import { editor } from "./editor.svelte";
import { t } from "./i18n.svelte";
import { pathMatrix } from "./plotter.svelte";
import { regenerate, worldPathD } from "./text.svelte";
import { aabb, center, rotate, uid } from "./geometry";
import { pathBbox, parsePath } from "./pathdata";
import { polysBox, weedLines, worldPolys } from "./weed";
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

// ---------- sheet tools (selection, or everything when nothing is selected) ----------

/** Rounded through-cut frame with 2 mm distance around the selection. */
export function frameSelection() {
  const b = aabb(editor.selected);
  if (!b) return;
  const pad = 2, r = 2;
  const w = b.x1 - b.x0 + 2 * pad, h = b.y1 - b.y0 + 2 * pad;
  const d = `M${r} 0H${w - r}A${r} ${r} 0 0 1 ${w} ${r}V${h - r}A${r} ${r} 0 0 1 ${w - r} ${h}H${r}A${r} ${r} 0 0 1 0 ${h - r}V${r}A${r} ${r} 0 0 1 ${r} 0Z`;
  const frame: PathObj = {
    id: uid(), type: "path", name: t("Schnittrahmen"), d, bbox: [0, 0, w, h], color: "#3e63dd", fill: false,
    x: b.x0 - pad, y: b.y0 - pad, w, h, rot: 0, flipX: false, flipY: false, visible: true, locked: false,
  };
  editor.add([frame], false);
  editor.toast("Rahmen zum Durchschneiden hinzugefügt", "ok");
}

export type WeedOptions = { margin: number; spacing: number; dir: "h" | "v" | "hv" | "none" };

/** Box around the selected design plus lines through the waste (vinyl). */
export function addWeeding(opt: WeedOptions) {
  const designs = selectedPaths().filter((o) => o.visible && o.cut !== false);
  if (!designs.length) return;
  const polys = worldPolys(designs);
  const b0 = polysBox(polys);
  if (!b0) return;
  const m = Math.max(0.5, opt.margin);
  const box = { x0: b0.x0 - m, y0: b0.y0 - m, x1: b0.x1 + m, y1: b0.y1 + m };
  const color = designs[0].color;
  const group = uid();
  const mk = (name: string, d: string): PathObj | null => {
    if (!d) return null;
    const [x, y, w, h] = pathBbox(parsePath(d));
    return {
      id: uid(), type: "path", name, d, bbox: [x, y, w || 0.01, h || 0.01], color, fill: false,
      x, y, w: w || 0.01, h: h || 0.01, rot: 0, flipX: false, flipY: false, visible: true, locked: false, group,
    };
  };
  const rect = `M${box.x0} ${box.y0}H${box.x1}V${box.y1}H${box.x0}Z`;
  const lines =
    opt.dir === "none"
      ? ""
      : weedLines(polys, box, Math.max(3, opt.spacing), Math.min(m, Math.max(0.5, opt.margin / 2)), {
          horizontal: opt.dir.includes("h"),
          vertical: opt.dir.includes("v"),
        });
  const objs = [mk(t("Entgitterungsrahmen"), rect), mk(t("Entgitterungslinien"), lines)].filter((o): o is PathObj => !!o);
  editor.add(objs, false);
  editor.toast(lines ? "Entgitterungsrahmen und -linien angelegt" : "Entgitterungsrahmen angelegt", "ok");
}

/** Selected objects split into units (a group or a single object). */
function selectedUnitsUnlocked(): Obj[][] {
  const ids = editor.expandGroups(editor.selection);
  const byKey = new Map<string, Obj[]>();
  for (const o of editor.objects) {
    if (!ids.includes(o.id) || o.locked) continue;
    const k = o.group ?? o.id;
    byKey.set(k, [...(byKey.get(k) ?? []), o]);
  }
  return [...byKey.values()];
}

/**
 * Fill the sheet with copies of the selection (`fill`) or arrange the
 * selected parts compactly. `shapes` = nest real outlines (with rotation),
 * otherwise a plain grid by bounding box (fill only).
 */
export async function nestSelection(fill: boolean, gap: number, shapes = true) {
  const units = selectedUnitsUnlocked();
  if (!units.length) return;
  if (fill && !shapes) return gridFill(gap);
  try {
    const { area, keepouts } = sheetArea();
    const items = (fill ? units.slice(0, 1) : units).map((u) => ({ paths: unitOutline(u) }));
    // Fill treats the whole selection as one unit.
    if (fill && units.length > 1) items[0] = { paths: units.flatMap(unitOutline) };
    const r = await api.nest({ items, area, keepouts, gap, rotate: true, fill });
    if (!r.placements.length) {
      editor.toast("Passt nicht auf den Bogen.", "error");
      return;
    }
    editor.checkpoint("Bogen füllen");
    if (fill) {
      const unit = units.flat();
      r.placements.forEach((p, i) => {
        // The original moves last, so copies are cloned from its start pose.
        if (i === 0) return;
        const groupMap = new Map<string, string>();
        const objs = unit.map((o) => {
          const c = { ...($state.snapshot(o) as Obj), id: uid() };
          if (c.group) c.group = groupMap.get(c.group) ?? groupMap.set(c.group, uid()).get(c.group);
          return c;
        });
        editor.objects.push(...objs);
        placeUnit(editor.objects.slice(-objs.length), p.rot, p.center, p.dx, p.dy);
      });
      placeUnit(unit, r.placements[0].rot, r.placements[0].center, r.placements[0].dx, r.placements[0].dy);
      editor.toast(t("{n} Exemplare platzsparend verteilt", { n: r.placements.length }), "ok");
    } else {
      for (const p of r.placements) placeUnit(units[p.item], p.rot, p.center, p.dx, p.dy);
      editor.toast(
        r.unplaced.length
          ? t("{a} angeordnet, {b} passen nicht mehr drauf", { a: r.placements.length, b: r.unplaced.length })
          : t("{n} Teile platzsparend angeordnet", { n: r.placements.length }),
        r.unplaced.length ? "error" : "ok",
      );
    }
  } catch (e) {
    editor.error(e);
  }
}

/** Simple grid fill by bounding box (no rotation). */
function gridFill(gap: number) {
  const unit = editor.selected.filter((o) => !o.locked);
  const b = aabb(unit);
  if (!b) return;
  const bw = b.x1 - b.x0, bh = b.y1 - b.y0;
  const r = editor.regmarks;
  const area = r
    ? { x0: r.originX + 1, y0: r.originY + 1, x1: r.originX + r.width - 1, y1: r.originY + r.length - 1 }
    : { x0: 5, y0: 5, x1: editor.mat.w - 5, y1: editor.mat.h - 5 };
  const zones = editor.regZones();
  const hits = (x: number, y: number) => zones.some(([zx, zy, zw, zh]) => x < zx + zw + 1 && x + bw > zx - 1 && y < zy + zh + 1 && y + bh > zy - 1);
  const cells: [number, number][] = [];
  for (let y = area.y0; y + bh <= area.y1; y += bh + gap) {
    for (let x = area.x0; x + bw <= area.x1; x += bw + gap) {
      if (!hits(x, y)) cells.push([x, y]);
    }
  }
  if (!cells.length) {
    editor.toast("Motiv ist zu groß für den Bogen.", "error");
    return;
  }
  editor.checkpoint("Bogen füllen");
  const [fx, fy] = cells[0];
  for (const o of unit) {
    o.x += fx - b.x0;
    o.y += fy - b.y0;
  }
  const copies: Obj[] = [];
  for (const [cx, cy] of cells.slice(1)) {
    const groupMap = new Map<string, string>();
    for (const o of unit) {
      const c = { ...($state.snapshot(o) as Obj), id: uid(), x: o.x + cx - fx, y: o.y + cy - fy };
      if (c.group) c.group = groupMap.get(c.group) ?? groupMap.set(c.group, uid()).get(c.group);
      copies.push(c);
    }
  }
  editor.objects.push(...copies);
  editor.toast(t("{n} Exemplare auf dem Bogen", { n: cells.length }), "ok");
}
