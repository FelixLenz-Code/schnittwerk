// Drawn shapes (rectangle, ellipse, polygon, star, heart, line, freehand):
// path generation in the object's box, so parameters stay editable.

import { ellipsePath, uid } from "./geometry";
import { parsePath, pathBbox } from "./pathdata";
import type { PathObj, ShapeSpec, ShapeTool, Vec2 } from "./types";

export const CUT_RED = "#e5484d";

const r3 = (v: number) => Math.round(v * 1000) / 1000;

/** Rounded rectangle (local units = mm). */
export function roundRect(w: number, h: number, r: number): string {
  r = Math.max(0, Math.min(r, w / 2, h / 2));
  if (r <= 0) return `M0 0H${r3(w)}V${r3(h)}H0Z`;
  const [W, H, R] = [r3(w), r3(h), r3(r)];
  return `M${R} 0H${r3(w - r)}A${R} ${R} 0 0 1 ${W} ${R}V${r3(h - r)}A${R} ${R} 0 0 1 ${r3(w - r)} ${H}H${R}A${R} ${R} 0 0 1 0 ${r3(h - r)}V${R}A${R} ${R} 0 0 1 ${R} 0Z`;
}

/** Corner points of a regular polygon / star on the unit circle, first point on top. */
function starPoints(spec: ShapeSpec): Vec2[] {
  const pts: Vec2[] = [];
  if (spec.kind === "polygon") {
    const n = Math.max(3, Math.round(spec.sides));
    for (let i = 0; i < n; i++) {
      const a = -Math.PI / 2 + (i * 2 * Math.PI) / n;
      pts.push({ x: Math.cos(a), y: Math.sin(a) });
    }
  } else if (spec.kind === "star") {
    const n = Math.max(3, Math.round(spec.points));
    const inner = Math.min(0.95, Math.max(0.1, spec.inner));
    for (let i = 0; i < 2 * n; i++) {
      const a = -Math.PI / 2 + (i * Math.PI) / n;
      const r = i % 2 ? inner : 1;
      pts.push({ x: Math.cos(a) * r, y: Math.sin(a) * r });
    }
  }
  return pts;
}

/** Fits points into the 0..w × 0..h box. */
function fitBox(pts: Vec2[], w: number, h: number): Vec2[] {
  const xs = pts.map((p) => p.x), ys = pts.map((p) => p.y);
  const x0 = Math.min(...xs), y0 = Math.min(...ys);
  const sx = w / (Math.max(...xs) - x0 || 1), sy = h / (Math.max(...ys) - y0 || 1);
  return pts.map((p) => ({ x: (p.x - x0) * sx, y: (p.y - y0) * sy }));
}

/** Heart in the unit box (cubic Béziers). */
const HEART: [number, number][][] = [
  [[0.5, 0.22]],
  [[0.42, 0.04], [0.29, 0], [0.22, 0]],
  [[0.08, 0], [0, 0.12], [0, 0.29]],
  [[0, 0.55], [0.3, 0.72], [0.5, 1]],
  [[0.7, 0.72], [1, 0.55], [1, 0.29]],
  [[1, 0.12], [0.92, 0], [0.78, 0]],
  [[0.71, 0], [0.58, 0.04], [0.5, 0.22]],
];

/** Height / width of the undistorted shape (Shift while drawing keeps it). */
export function naturalAspect(spec: ShapeSpec): number {
  if (spec.kind === "polygon" || spec.kind === "star") {
    const pts = starPoints(spec);
    const xs = pts.map((p) => p.x), ys = pts.map((p) => p.y);
    return (Math.max(...ys) - Math.min(...ys)) / (Math.max(...xs) - Math.min(...xs));
  }
  if (spec.kind === "heart") return 0.92;
  return 1;
}

export function shapePath(spec: ShapeSpec, w: number, h: number): string {
  switch (spec.kind) {
    case "rect":
      return roundRect(w, h, spec.radius);
    case "ellipse":
      return ellipsePath(w, h);
    case "heart": {
      const p = ([x, y]: [number, number]) => `${r3(x * w)} ${r3(y * h)}`;
      return `M${p(HEART[0][0])}` + HEART.slice(1).map((c) => `C${c.map(p).join(" ")}`).join("") + "Z";
    }
    default: {
      const pts = fitBox(starPoints(spec), w, h);
      return "M" + pts.map((q) => `${r3(q.x)} ${r3(q.y)}`).join("L") + "Z";
    }
  }
}

export const SHAPE_NAMES: Record<ShapeSpec["kind"], string> = {
  rect: "Rechteck",
  ellipse: "Ellipse",
  polygon: "Vieleck",
  star: "Stern",
  heart: "Herz",
};

/** Starting parameters for a newly drawn shape (remembered per kind). */
const lastSpec: Partial<Record<ShapeTool, ShapeSpec>> = {};

export function defaultSpec(tool: ShapeTool): ShapeSpec {
  if (lastSpec[tool]) return structuredClone(lastSpec[tool]!);
  switch (tool) {
    case "rect":
      return { kind: "rect", radius: 0 };
    case "ellipse":
      return { kind: "ellipse" };
    case "polygon":
      return { kind: "polygon", sides: 6 };
    case "star":
      return { kind: "star", points: 5, inner: 0.45 };
    case "heart":
      return { kind: "heart" };
  }
}

export function rememberSpec(spec: ShapeSpec) {
  const tool = spec.kind as ShapeTool;
  lastSpec[tool] = structuredClone(spec);
}

export function makeShape(spec: ShapeSpec, x: number, y: number, w: number, h: number): PathObj {
  return {
    id: uid(), type: "path", name: SHAPE_NAMES[spec.kind], d: shapePath(spec, w, h), bbox: [0, 0, w, h],
    color: CUT_RED, fill: false, x, y, w, h, rot: 0, flipX: false, flipY: false, visible: true, locked: false,
    shape: spec,
  };
}

/** Regenerates a shape's path at its current size (keeps corner radii round). */
export function refreshShape(o: PathObj) {
  if (!o.shape || o.w <= 0 || o.h <= 0) return;
  o.d = shapePath(o.shape, o.w, o.h);
  o.bbox = [0, 0, o.w, o.h];
}

/** Path object from path data given in mat coordinates. */
function worldPathObj(name: string, d: string): PathObj {
  const [x, y, w0, h0] = pathBbox(parsePath(d));
  const w = Math.max(w0, 0.01), h = Math.max(h0, 0.01);
  return {
    id: uid(), type: "path", name, d, bbox: [x, y, w, h], color: CUT_RED, fill: false,
    x, y, w, h, rot: 0, flipX: false, flipY: false, visible: true, locked: false,
  };
}

export function makeLine(a: Vec2, b: Vec2): PathObj {
  return worldPathObj("Linie", `M${r3(a.x)} ${r3(a.y)}L${r3(b.x)} ${r3(b.y)}`);
}

/** Douglas–Peucker on a polyline. */
function rdp(pts: Vec2[], tol: number): Vec2[] {
  if (pts.length < 3) return pts;
  const a = pts[0], b = pts[pts.length - 1];
  const dx = b.x - a.x, dy = b.y - a.y, len = Math.hypot(dx, dy);
  let idx = 0, max = 0;
  for (let i = 1; i < pts.length - 1; i++) {
    const p = pts[i];
    const d = len < 1e-9 ? Math.hypot(p.x - a.x, p.y - a.y) : Math.abs(dx * (p.y - a.y) - dy * (p.x - a.x)) / len;
    if (d > max) {
      max = d;
      idx = i;
    }
  }
  if (max <= tol) return [a, b];
  return [...rdp(pts.slice(0, idx + 1), tol).slice(0, -1), ...rdp(pts.slice(idx), tol)];
}

/**
 * Freehand stroke → smooth path: thinned out, then a Catmull-Rom spline
 * through the remaining points. `close` joins the end to the start.
 */
export function makeFreehand(raw: Vec2[], close: boolean, tol = 0.3): PathObj | null {
  let pts = rdp(raw, tol);
  if (close && pts.length > 2) {
    pts = pts.slice(0, -1);
    if (Math.hypot(pts[0].x - pts[pts.length - 1].x, pts[0].y - pts[pts.length - 1].y) < tol) pts.pop();
  }
  if (pts.length < 2) return null;
  const n = pts.length;
  const at = (i: number) => (close ? pts[(i + n) % n] : pts[Math.max(0, Math.min(n - 1, i))]);
  const f = (v: Vec2) => `${r3(v.x)} ${r3(v.y)}`;
  let d = `M${f(pts[0])}`;
  const segs = close ? n : n - 1;
  for (let i = 0; i < segs; i++) {
    const p0 = at(i - 1), p1 = at(i), p2 = at(i + 1), p3 = at(i + 2);
    const c1 = { x: p1.x + (p2.x - p0.x) / 6, y: p1.y + (p2.y - p0.y) / 6 };
    const c2 = { x: p2.x - (p3.x - p1.x) / 6, y: p2.y - (p3.y - p1.y) / 6 };
    d += `C${f(c1)} ${f(c2)} ${f(p2)}`;
  }
  if (close) d += "Z";
  return worldPathObj("Freihand", d);
}
