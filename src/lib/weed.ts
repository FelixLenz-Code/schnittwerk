// Weeding aids for vinyl: a box around the design and lines through the
// waste area, so the excess foil comes off in strips.

import { flattenPath, parsePath } from "./pathdata";
import { pathMatrix } from "./plotter.svelte";
import type { PathObj, Vec2 } from "./types";

type Poly = { pts: Vec2[]; closed: boolean };

/** Outlines of the objects in mat coordinates (mm). */
export function worldPolys(objs: PathObj[]): Poly[][] {
  return objs.map((o) => {
    const [a, b, c, d, e, f] = pathMatrix(o, null);
    return flattenPath(parsePath(o.d), 0.1).map((p) => ({
      closed: p.closed,
      pts: p.pts.map((q) => ({ x: a * q.x + c * q.y + e, y: b * q.x + d * q.y + f })),
    }));
  });
}

export function polysBox(polys: Poly[][]): { x0: number; y0: number; x1: number; y1: number } | null {
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  for (const obj of polys)
    for (const p of obj)
      for (const q of p.pts) {
        x0 = Math.min(x0, q.x);
        y0 = Math.min(y0, q.y);
        x1 = Math.max(x1, q.x);
        y1 = Math.max(y1, q.y);
      }
  return isFinite(x0) ? { x0, y0, x1, y1 } : null;
}

type Interval = [number, number];

function merge(list: Interval[]): Interval[] {
  list.sort((a, b) => a[0] - b[0]);
  const out: Interval[] = [];
  for (const iv of list) {
    const last = out[out.length - 1];
    if (last && iv[0] <= last[1]) last[1] = Math.max(last[1], iv[1]);
    else out.push([iv[0], iv[1]]);
  }
  return out;
}

/**
 * Blocked stretches of the horizontal line `y` (x intervals): inside a
 * design (even-odd per object, so letter counters stay free) or closer
 * than `margin` to any outline.
 */
function blocked(polys: Poly[][], y: number, margin: number): Interval[] {
  const out: Interval[] = [];
  for (const obj of polys) {
    // Inside: crossings of the closed outlines of this object.
    for (const yy of [y - margin, y, y + margin]) {
      const xs: number[] = [];
      for (const p of obj) {
        if (!p.closed) continue;
        const n = p.pts.length;
        for (let i = 0; i < n; i++) {
          const a = p.pts[i], b = p.pts[(i + 1) % n];
          if (a.y <= yy !== b.y <= yy) xs.push(a.x + ((yy - a.y) / (b.y - a.y)) * (b.x - a.x));
        }
      }
      xs.sort((a, b) => a - b);
      for (let i = 0; i + 1 < xs.length; i += 2) out.push([xs[i] - margin, xs[i + 1] + margin]);
    }
    // Near: every outline segment that comes within the band.
    for (const p of obj) {
      const n = p.pts.length;
      const count = p.closed ? n : n - 1;
      for (let i = 0; i < count; i++) {
        const a = p.pts[i], b = p.pts[(i + 1) % n];
        const lo = y - margin, hi = y + margin;
        if (Math.max(a.y, b.y) < lo || Math.min(a.y, b.y) > hi) continue;
        // x range of the part of segment a→b inside the band.
        let t0 = 0, t1 = 1;
        if (b.y !== a.y) {
          const ta = (lo - a.y) / (b.y - a.y), tb = (hi - a.y) / (b.y - a.y);
          t0 = Math.max(0, Math.min(ta, tb));
          t1 = Math.min(1, Math.max(ta, tb));
        }
        const xa = a.x + (b.x - a.x) * t0, xb = a.x + (b.x - a.x) * t1;
        out.push([Math.min(xa, xb) - margin, Math.max(xa, xb) + margin]);
      }
    }
  }
  return merge(out);
}

/** Free stretches of the line between x0 and x1, at least `minLen` long. */
function free(block: Interval[], x0: number, x1: number, minLen: number): Interval[] {
  const out: Interval[] = [];
  let cur = x0;
  for (const [a, b] of block) {
    if (b <= cur) continue;
    if (a >= x1) break;
    if (a - cur >= minLen) out.push([cur, a]);
    cur = Math.max(cur, b);
  }
  if (x1 - cur >= minLen) out.push([cur, x1]);
  return out;
}

const r = (v: number) => Math.round(v * 1000) / 1000;

/**
 * Weed lines inside the box (mat mm) as path data. `spacing` = distance
 * between lines, `margin` = distance kept from the design.
 */
export function weedLines(
  polys: Poly[][],
  box: { x0: number; y0: number; x1: number; y1: number },
  spacing: number,
  margin: number,
  dirs: { horizontal: boolean; vertical: boolean },
): string {
  const parts: string[] = [];
  const minLen = Math.max(1, margin);
  const run = (vertical: boolean) => {
    // Vertical lines: same algorithm on the mirrored (x ↔ y) geometry.
    const P = vertical ? polys.map((o) => o.map((p) => ({ closed: p.closed, pts: p.pts.map((q) => ({ x: q.y, y: q.x })) }))) : polys;
    const [a0, a1, b0, b1] = vertical ? [box.y0, box.y1, box.x0, box.x1] : [box.x0, box.x1, box.y0, box.y1];
    const n = Math.floor((b1 - b0) / spacing);
    // Centre the lines in the box.
    const first = b0 + ((b1 - b0) - (n - 1) * spacing) / 2;
    for (let k = 0; k < n; k++) {
      const c = first + k * spacing;
      if (c <= b0 + margin / 2 || c >= b1 - margin / 2) continue;
      for (const [s, e] of free(blocked(P, c, margin), a0, a1, minLen)) {
        parts.push(vertical ? `M${r(c)} ${r(s)}L${r(c)} ${r(e)}` : `M${r(s)} ${r(c)}L${r(e)} ${r(c)}`);
      }
    }
  };
  if (dirs.horizontal) run(false);
  if (dirs.vertical) run(true);
  return parts.join(" ");
}
