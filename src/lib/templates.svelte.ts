// Project templates: ready-made sheets (stickers, labels, cards, tags).

import { editor, MATS } from "./editor.svelte";
import { uid } from "./geometry";
import { confirmDiscard } from "./io.svelte";
import type { PathObj } from "./types";

export const templatesUi = $state({ open: false });

const KISS = "#e5484d";
const THROUGH = "#3e63dd";
const GUIDE = "#8b919c";

/** Rounded rectangle path (local units = mm). */
function roundRect(w: number, h: number, r: number): string {
  r = Math.min(r, w / 2, h / 2);
  if (r <= 0) return `M0 0H${w}V${h}H0Z`;
  return `M${r} 0H${w - r}A${r} ${r} 0 0 1 ${w} ${r}V${h - r}A${r} ${r} 0 0 1 ${w - r} ${h}H${r}A${r} ${r} 0 0 1 0 ${h - r}V${r}A${r} ${r} 0 0 1 ${r} 0Z`;
}

function circle(dia: number): string {
  const r = dia / 2;
  return `M0 ${r}A${r} ${r} 0 1 0 ${dia} ${r}A${r} ${r} 0 1 0 0 ${r}Z`;
}

/** Tag with clipped top corners and a hole. */
function tag(w: number, h: number): string {
  const c = w * 0.28, hr = w * 0.07, hy = c * 0.75;
  return `M${c} 0H${w - c}L${w} ${c}V${h}H0V${c}Z M${w / 2 - hr} ${hy}A${hr} ${hr} 0 1 0 ${w / 2 + hr} ${hy}A${hr} ${hr} 0 1 0 ${w / 2 - hr} ${hy}Z`;
}

function bookmark(w: number, h: number): string {
  const hr = 2.5, hy = 10;
  return `${roundRect(w, h, 4)} M${w / 2 - hr} ${hy}A${hr} ${hr} 0 1 0 ${w / 2 + hr} ${hy}A${hr} ${hr} 0 1 0 ${w / 2 - hr} ${hy}Z`;
}

function path(name: string, d: string, w: number, h: number, x: number, y: number, color: string, extra: Partial<PathObj> = {}): PathObj {
  return { id: uid(), type: "path", name, d, bbox: [0, 0, w, h], color, fill: false, x, y, w, h, rot: 0, flipX: false, flipY: false, visible: true, locked: false, ...extra };
}

/** Area that may be used: inside the mark frame (print & cut) or the mat. */
function usable(margin: number): { x0: number; y0: number; x1: number; y1: number } {
  const r = editor.regmarks;
  if (r) return { x0: r.originX + 2, y0: r.originY + 2, x1: r.originX + r.width - 2, y1: r.originY + r.length - 2 };
  return { x0: margin, y0: margin, x1: editor.mat.w - margin, y1: editor.mat.h - margin };
}

/** Places w×h items in a centred grid, skipping the registration zones. */
function grid(w: number, h: number, gap: number, make: (x: number, y: number, i: number) => PathObj[], margin = 10): PathObj[] {
  const a = usable(margin);
  const cols = Math.floor((a.x1 - a.x0 + gap) / (w + gap));
  const rows = Math.floor((a.y1 - a.y0 + gap) / (h + gap));
  const ox = a.x0 + (a.x1 - a.x0 - (cols * (w + gap) - gap)) / 2;
  const oy = a.y0 + (a.y1 - a.y0 - (rows * (h + gap) - gap)) / 2;
  const zones = editor.regZones();
  const out: PathObj[] = [];
  let i = 0;
  for (let r = 0; r < rows; r++)
    for (let c = 0; c < cols; c++) {
      const x = ox + c * (w + gap), y = oy + r * (h + gap);
      if (zones.some(([zx, zy, zw, zh]) => x < zx + zw && x + w > zx && y < zy + zh && y + h > zy)) continue;
      out.push(...make(x, y, i++));
    }
  return out;
}

export interface Template {
  id: string;
  name: string;
  hint: string;
  /** Print & cut page, or null for the cutting mat. */
  printCut: boolean;
  build: () => PathObj[];
}

export const TEMPLATES: Template[] = [
  {
    id: "empty",
    name: "Leere Matte",
    hint: "Portrait 3 · 8″ × 12″",
    printCut: false,
    build: () => [],
  },
  {
    id: "stickers-round",
    name: "Sticker rund 50 mm",
    hint: "Print & Cut A4 · angeschnitten, Bild per Drag & Drop in die Kreise legen",
    printCut: true,
    build: () => grid(50, 50, 6, (x, y, i) => [path(`Sticker ${i + 1}`, circle(50), 50, 50, x, y, KISS)]),
  },
  {
    id: "labels",
    name: "Etiketten 63,5 × 38,1 mm",
    hint: "Print & Cut A4 · abgerundete Etiketten zum Beschriften und Bedrucken",
    printCut: true,
    build: () => grid(63.5, 38.1, 4, (x, y, i) => [path(`Etikett ${i + 1}`, roundRect(63.5, 38.1, 3), 63.5, 38.1, x, y, KISS)]),
  },
  {
    id: "labels-round",
    name: "Rundetiketten 40 mm",
    hint: "Print & Cut A4 · z. B. für Gläser und Dosen",
    printCut: true,
    build: () => grid(40, 40, 5, (x, y, i) => [path(`Etikett ${i + 1}`, circle(40), 40, 40, x, y, KISS)]),
  },
  {
    id: "card",
    name: "Klappkarte A6",
    hint: "Karton auf der Matte · A5 quer, in der Mitte zu A6 falten (Falzlinie wird nicht geschnitten)",
    printCut: false,
    build: () => {
      const w = 148, h = 210, x = (MATS[0].w - w) / 2, y = 20;
      return [
        path("Klappkarte", roundRect(w, h, 0), w, h, x, y, KISS),
        path("Falzlinie", `M0 0H${w}`, w, 0.01, x, y + h / 2, GUIDE, { cut: false, bbox: [0, 0, w, 0.01] }),
      ];
    },
  },
  {
    id: "bookmarks",
    name: "Lesezeichen 50 × 180 mm",
    hint: "Karton auf der Matte · mit Loch für ein Band",
    printCut: false,
    build: () => grid(50, 180, 8, (x, y, i) => [path(`Lesezeichen ${i + 1}`, bookmark(50, 180), 50, 180, x, y, KISS)], 12),
  },
  {
    id: "tags",
    name: "Geschenkanhänger",
    hint: "Karton auf der Matte · 50 × 80 mm mit Loch",
    printCut: false,
    build: () => grid(50, 80, 6, (x, y, i) => [path(`Anhänger ${i + 1}`, tag(50, 80), 50, 80, x, y, KISS)], 12),
  },
  {
    id: "sticker-sheet-frame",
    name: "Sticker-Bogen mit Rahmen",
    hint: "Print & Cut A4 · leerer Bogen mit durchgeschnittenem Rahmen – eigene Motive hineinlegen",
    printCut: true,
    build: () => {
      const a = usable(10);
      const w = a.x1 - a.x0 - 4, h = a.y1 - a.y0 - 4;
      return [path("Bogenrahmen", roundRect(w, h, 4), w, h, a.x0 + 2, a.y0 + 2, THROUGH)];
    },
  },
];

/** Starts a new project from a template (asks before discarding changes). */
export async function newFromTemplate(t: Template) {
  if (!(await confirmDiscard())) return;
  editor.objects = [];
  editor.images = {};
  editor.selection = [];
  editor.tracePreview = null;
  editor.filePath = null;
  editor.setPrintCut(t.printCut, "A4");
  const objs = t.build();
  if (objs.length) editor.objects.push(...objs);
  editor.resetHistory();
  editor.dirty = false;
  templatesUi.open = false;
}
