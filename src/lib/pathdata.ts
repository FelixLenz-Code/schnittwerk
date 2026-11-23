// SVG path data as editable nodes: parsing (all commands, arcs become
// cubics), serializing, bounding boxes, splitting and flattening.

import type { Bbox, Vec2 } from "./types";

/** A path node; `cin` / `cout` are the control points before / after it
 *  (absent = straight segment on that side). */
export interface PNode {
  x: number;
  y: number;
  cin?: Vec2;
  cout?: Vec2;
  /** Handles stay collinear when one is dragged. */
  smooth?: boolean;
}

export interface Sub {
  nodes: PNode[];
  closed: boolean;
}

const TOKEN = /([MmLlHhVvCcSsQqTtAaZz])|(-?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?)/g;

/** Parses path data into absolute subpaths of lines and cubics. */
export function parsePath(d: string): Sub[] {
  const toks: (string | number)[] = [];
  for (const m of d.matchAll(TOKEN)) toks.push(m[1] ?? Number(m[2]));
  const subs: Sub[] = [];
  let sub: Sub | null = null;
  let cx = 0, cy = 0, sx = 0, sy = 0;
  let lastCtrl: Vec2 | null = null; // reflection source for S / T
  let lastQ: Vec2 | null = null;
  let cmd = "";
  let i = 0;
  const num = () => {
    const t = toks[i++];
    if (typeof t !== "number") throw new Error("Ungültige Pfaddaten");
    return t;
  };
  const start = (x: number, y: number) => {
    sub = { nodes: [{ x, y }], closed: false };
    subs.push(sub);
  };
  const lineTo = (x: number, y: number) => {
    if (!sub) start(cx, cy);
    sub!.nodes.push({ x, y });
  };
  const cubicTo = (c1: Vec2, c2: Vec2, x: number, y: number) => {
    if (!sub) start(cx, cy);
    const prev = sub!.nodes[sub!.nodes.length - 1];
    prev.cout = c1;
    sub!.nodes.push({ x, y, cin: c2 });
  };
  while (i < toks.length) {
    const t = toks[i];
    if (typeof t === "string") {
      cmd = t;
      i++;
    } else if (!cmd) {
      throw new Error("Ungültige Pfaddaten");
    }
    const rel = cmd === cmd.toLowerCase() && cmd !== "z" ? true : false;
    const C = cmd.toUpperCase();
    const ox = rel ? cx : 0, oy = rel ? cy : 0;
    let ctrl: Vec2 | null = null, q: Vec2 | null = null;
    switch (C) {
      case "M": {
        const x = num() + ox, y = num() + oy;
        start(x, y);
        cx = sx = x;
        cy = sy = y;
        cmd = rel ? "l" : "L"; // further pairs are line-tos
        break;
      }
      case "L": {
        const x = num() + ox, y = num() + oy;
        lineTo(x, y);
        cx = x;
        cy = y;
        break;
      }
      case "H": {
        const x = num() + ox;
        lineTo(x, cy);
        cx = x;
        break;
      }
      case "V": {
        const y = num() + oy;
        lineTo(cx, y);
        cy = y;
        break;
      }
      case "C": {
        const c1 = { x: num() + ox, y: num() + oy };
        const c2 = { x: num() + ox, y: num() + oy };
        const x = num() + ox, y = num() + oy;
        cubicTo(c1, c2, x, y);
        ctrl = c2;
        cx = x;
        cy = y;
        break;
      }
      case "S": {
        const c1 = lastCtrl ? { x: 2 * cx - lastCtrl.x, y: 2 * cy - lastCtrl.y } : { x: cx, y: cy };
        const c2 = { x: num() + ox, y: num() + oy };
        const x = num() + ox, y = num() + oy;
        cubicTo(c1, c2, x, y);
        ctrl = c2;
        cx = x;
        cy = y;
        break;
      }
      case "Q":
      case "T": {
        const qc: Vec2 =
          C === "Q" ? { x: num() + ox, y: num() + oy } : lastQ ? { x: 2 * cx - lastQ.x, y: 2 * cy - lastQ.y } : { x: cx, y: cy };
        const x = num() + ox, y = num() + oy;
        cubicTo({ x: cx + (2 / 3) * (qc.x - cx), y: cy + (2 / 3) * (qc.y - cy) }, { x: x + (2 / 3) * (qc.x - x), y: y + (2 / 3) * (qc.y - y) }, x, y);
        q = qc;
        cx = x;
        cy = y;
        break;
      }
      case "A": {
        const rx = num(), ry = num(), rot = num(), large = num(), sweep = num();
        const x = num() + ox, y = num() + oy;
        for (const [c1, c2, e] of arcToCubics(cx, cy, rx, ry, rot, !!large, !!sweep, x, y)) cubicTo(c1, c2, e.x, e.y);
        cx = x;
        cy = y;
        break;
      }
      case "Z": {
        if (sub) {
          const s = sub as Sub;
          s.closed = true;
          // A closing node on the start point is merged into it.
          const first = s.nodes[0], last = s.nodes[s.nodes.length - 1];
          if (s.nodes.length > 1 && Math.abs(first.x - last.x) < 1e-9 && Math.abs(first.y - last.y) < 1e-9) {
            first.cin = last.cin;
            s.nodes.pop();
          }
        }
        sub = null;
        cx = sx;
        cy = sy;
        break;
      }
      default:
        throw new Error("Ungültige Pfaddaten");
    }
    lastCtrl = ctrl;
    lastQ = q;
  }
  return subs.filter((s) => s.nodes.length > 0);
}

/** Endpoint arc → cubic Béziers (each ≤ 90°). */
function arcToCubics(x1: number, y1: number, rx: number, ry: number, rotDeg: number, large: boolean, sweep: boolean, x2: number, y2: number): [Vec2, Vec2, Vec2][] {
  if ((x1 === x2 && y1 === y2) || rx === 0 || ry === 0) {
    return [[{ x: x1, y: y1 }, { x: x2, y: y2 }, { x: x2, y: y2 }]];
  }
  const phi = (rotDeg * Math.PI) / 180;
  const cos = Math.cos(phi), sin = Math.sin(phi);
  const dx = (x1 - x2) / 2, dy = (y1 - y2) / 2;
  const x1p = cos * dx + sin * dy, y1p = -sin * dx + cos * dy;
  rx = Math.abs(rx);
  ry = Math.abs(ry);
  const lam = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
  if (lam > 1) {
    rx *= Math.sqrt(lam);
    ry *= Math.sqrt(lam);
  }
  const num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p;
  const den = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
  let co = Math.sqrt(Math.max(0, num / den));
  if (large === sweep) co = -co;
  const cxp = (co * rx * y1p) / ry, cyp = (-co * ry * x1p) / rx;
  const cxa = cos * cxp - sin * cyp + (x1 + x2) / 2;
  const cya = sin * cxp + cos * cyp + (y1 + y2) / 2;
  const ang = (ux: number, uy: number, vx: number, vy: number) => {
    const a = Math.atan2(ux * vy - uy * vx, ux * vx + uy * vy);
    return a;
  };
  const t1 = ang(1, 0, (x1p - cxp) / rx, (y1p - cyp) / ry);
  let dt = ang((x1p - cxp) / rx, (y1p - cyp) / ry, (-x1p - cxp) / rx, (-y1p - cyp) / ry);
  if (!sweep && dt > 0) dt -= 2 * Math.PI;
  if (sweep && dt < 0) dt += 2 * Math.PI;
  const n = Math.max(1, Math.ceil(Math.abs(dt) / (Math.PI / 2) - 1e-9));
  const step = dt / n;
  const k = (4 / 3) * Math.tan(step / 4);
  const pt = (t: number) => {
    const ex = rx * Math.cos(t), ey = ry * Math.sin(t);
    return { x: cxa + cos * ex - sin * ey, y: cya + sin * ex + cos * ey };
  };
  const der = (t: number) => {
    const ex = -rx * Math.sin(t), ey = ry * Math.cos(t);
    return { x: cos * ex - sin * ey, y: sin * ex + cos * ey };
  };
  const out: [Vec2, Vec2, Vec2][] = [];
  for (let s = 0; s < n; s++) {
    const a = t1 + s * step, b = a + step;
    const p0 = pt(a), p1 = pt(b), d0 = der(a), d1 = der(b);
    out.push([
      { x: p0.x + k * d0.x, y: p0.y + k * d0.y },
      { x: p1.x - k * d1.x, y: p1.y - k * d1.y },
      s === n - 1 ? { x: x2, y: y2 } : p1,
    ]);
  }
  return out;
}

const r = (v: number) => {
  const s = (Math.round(v * 1000) / 1000).toString();
  return s === "-0" ? "0" : s;
};

/** Segment from node a to node b: [p0, c1, c2, p1] or null for a line. */
export function segment(a: PNode, b: PNode): [Vec2, Vec2, Vec2, Vec2] | null {
  if (!a.cout && !b.cin) return null;
  return [a, a.cout ?? a, b.cin ?? b, b];
}

export function serializePath(subs: Sub[]): string {
  const parts: string[] = [];
  for (const s of subs) {
    if (!s.nodes.length) continue;
    const n = s.nodes;
    parts.push(`M${r(n[0].x)} ${r(n[0].y)}`);
    const count = s.closed ? n.length : n.length - 1;
    for (let i = 0; i < count; i++) {
      const a = n[i], b = n[(i + 1) % n.length];
      const seg = segment(a, b);
      if (seg) parts.push(`C${r(seg[1].x)} ${r(seg[1].y)} ${r(seg[2].x)} ${r(seg[2].y)} ${r(b.x)} ${r(b.y)}`);
      else if (!(s.closed && i === n.length - 1)) parts.push(`L${r(b.x)} ${r(b.y)}`);
    }
    if (s.closed) parts.push("Z");
  }
  return parts.join(" ");
}

function cubicAt(p: [Vec2, Vec2, Vec2, Vec2], t: number): Vec2 {
  const u = 1 - t;
  const a = u * u * u, b = 3 * u * u * t, c = 3 * u * t * t, d = t * t * t;
  return { x: a * p[0].x + b * p[1].x + c * p[2].x + d * p[3].x, y: a * p[0].y + b * p[1].y + c * p[2].y + d * p[3].y };
}

/** Parameters in (0,1) where a cubic has an extremum on one axis. */
function extrema(a: number, b: number, c: number, d: number): number[] {
  // Derivative: 3[(b-a)(1-t)² + 2(c-b)(1-t)t + (d-c)t²] = qa t² + qb t + qc
  const qa = -a + 3 * b - 3 * c + d, qb = 2 * (a - 2 * b + c), qc = b - a;
  const out: number[] = [];
  if (Math.abs(qa) < 1e-12) {
    if (Math.abs(qb) > 1e-12) out.push(-qc / qb);
  } else {
    const disc = qb * qb - 4 * qa * qc;
    if (disc >= 0) {
      const sq = Math.sqrt(disc);
      out.push((-qb + sq) / (2 * qa), (-qb - sq) / (2 * qa));
    }
  }
  return out.filter((t) => t > 0 && t < 1);
}

/** Exact bounding box of the path. */
export function pathBbox(subs: Sub[]): Bbox {
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  const add = (p: Vec2) => {
    x0 = Math.min(x0, p.x);
    y0 = Math.min(y0, p.y);
    x1 = Math.max(x1, p.x);
    y1 = Math.max(y1, p.y);
  };
  for (const s of subs) {
    const n = s.nodes;
    n.forEach(add);
    const count = s.closed ? n.length : n.length - 1;
    for (let i = 0; i < count; i++) {
      const seg = segment(n[i], n[(i + 1) % n.length]);
      if (!seg) continue;
      for (const t of [...extrema(seg[0].x, seg[1].x, seg[2].x, seg[3].x), ...extrema(seg[0].y, seg[1].y, seg[2].y, seg[3].y)]) add(cubicAt(seg, t));
    }
  }
  if (!isFinite(x0)) return [0, 0, 0, 0];
  return [x0, y0, x1 - x0, y1 - y0];
}

/** Splits segment i → i+1 of a subpath at parameter t (inserts a node). */
export function splitSegment(s: Sub, i: number, t: number): number {
  const n = s.nodes;
  const a = n[i], b = n[(i + 1) % n.length];
  const seg = segment(a, b);
  if (!seg) {
    n.splice(i + 1, 0, { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t });
    return i + 1;
  }
  const lerp = (p: Vec2, q: Vec2) => ({ x: p.x + (q.x - p.x) * t, y: p.y + (q.y - p.y) * t });
  const [p0, p1, p2, p3] = seg;
  const p01 = lerp(p0, p1), p12 = lerp(p1, p2), p23 = lerp(p2, p3);
  const p012 = lerp(p01, p12), p123 = lerp(p12, p23);
  const m = lerp(p012, p123);
  a.cout = p01;
  b.cin = p23;
  n.splice(i + 1, 0, { x: m.x, y: m.y, cin: p012, cout: p123, smooth: true });
  return i + 1;
}

/** Polyline points of every subpath (curves sampled by `tol`). */
export function flattenPath(subs: Sub[], tol = 0.2): { pts: Vec2[]; closed: boolean }[] {
  return subs.map((s) => {
    const n = s.nodes;
    const pts: Vec2[] = n.length ? [{ x: n[0].x, y: n[0].y }] : [];
    const count = s.closed ? n.length : n.length - 1;
    for (let i = 0; i < count; i++) {
      const a = n[i], b = n[(i + 1) % n.length];
      const seg = segment(a, b);
      if (!seg) {
        pts.push({ x: b.x, y: b.y });
        continue;
      }
      const len = Math.hypot(seg[1].x - seg[0].x, seg[1].y - seg[0].y) + Math.hypot(seg[2].x - seg[1].x, seg[2].y - seg[1].y) + Math.hypot(seg[3].x - seg[2].x, seg[3].y - seg[2].y);
      const steps = Math.max(2, Math.min(200, Math.ceil(Math.sqrt(len / tol))));
      for (let k = 1; k <= steps; k++) pts.push(cubicAt(seg, k / steps));
    }
    return { pts, closed: s.closed };
  });
}

/** Closest point on the path to p: subpath, segment index, parameter, distance. */
export function nearestSegment(subs: Sub[], p: Vec2): { si: number; i: number; t: number; dist: number } | null {
  let best: { si: number; i: number; t: number; dist: number } | null = null;
  subs.forEach((s, si) => {
    const n = s.nodes;
    const count = s.closed ? n.length : n.length - 1;
    for (let i = 0; i < count; i++) {
      const a = n[i], b = n[(i + 1) % n.length];
      const seg = segment(a, b) ?? [a, a, b, b];
      const steps = 40;
      for (let k = 0; k <= steps; k++) {
        const t = k / steps;
        const q = cubicAt(seg as [Vec2, Vec2, Vec2, Vec2], t);
        const dist = Math.hypot(q.x - p.x, q.y - p.y);
        if (!best || dist < best.dist) best = { si, i, t, dist };
      }
    }
  });
  if (best) {
    // Avoid inserting on top of an existing node.
    const b = best as { si: number; i: number; t: number; dist: number };
    b.t = Math.min(0.98, Math.max(0.02, b.t));
  }
  return best;
}
