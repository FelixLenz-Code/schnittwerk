import type { Obj, Vec2 } from "./types";

export const DEG = Math.PI / 180;

export function rotate(v: Vec2, deg: number): Vec2 {
  const c = Math.cos(deg * DEG);
  const s = Math.sin(deg * DEG);
  return { x: v.x * c - v.y * s, y: v.x * s + v.y * c };
}

export function center(o: Obj): Vec2 {
  return { x: o.x + o.w / 2, y: o.y + o.h / 2 };
}

/** World positions of the four box corners (TL, TR, BR, BL). */
export function corners(o: Obj): Vec2[] {
  const c = center(o);
  return [
    { x: -o.w / 2, y: -o.h / 2 },
    { x: o.w / 2, y: -o.h / 2 },
    { x: o.w / 2, y: o.h / 2 },
    { x: -o.w / 2, y: o.h / 2 },
  ].map((p) => {
    const r = rotate(p, o.rot);
    return { x: c.x + r.x, y: c.y + r.y };
  });
}

export interface AABB {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

export function aabb(objs: Obj[]): AABB | null {
  if (!objs.length) return null;
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  for (const o of objs) {
    for (const p of corners(o)) {
      x0 = Math.min(x0, p.x);
      y0 = Math.min(y0, p.y);
      x1 = Math.max(x1, p.x);
      y1 = Math.max(y1, p.y);
    }
  }
  return { x0, y0, x1, y1 };
}

/** Converts a world point into the object's unrotated local box coords (0..w, 0..h). */
export function toLocal(o: Obj, p: Vec2): Vec2 {
  const c = center(o);
  const r = rotate({ x: p.x - c.x, y: p.y - c.y }, -o.rot);
  let x = r.x, y = r.y;
  if (o.flipX) x = -x;
  if (o.flipY) y = -y;
  return { x: x + o.w / 2, y: y + o.h / 2 };
}

/** SVG transform that maps the 0..w × 0..h box onto the mat. */
export function boxTransform(o: Obj): string {
  let t = `translate(${o.x} ${o.y}) rotate(${o.rot} ${o.w / 2} ${o.h / 2})`;
  if (o.flipX || o.flipY) {
    t += ` translate(${o.w / 2} ${o.h / 2}) scale(${o.flipX ? -1 : 1} ${o.flipY ? -1 : 1}) translate(${-o.w / 2} ${-o.h / 2})`;
  }
  return t;
}

export function fmt(v: number, digits = 1): string {
  return (Math.round(v * 10 ** digits) / 10 ** digits).toLocaleString("de-DE", {
    minimumFractionDigits: 0,
    maximumFractionDigits: digits,
  });
}

export function uid(): string {
  return Math.random().toString(36).slice(2, 10);
}

export function rectPath(w: number, h: number): string {
  return `M0 0 L${w} 0 L${w} ${h} L0 ${h} Z`;
}

export function ellipsePath(w: number, h: number): string {
  const rx = w / 2, ry = h / 2;
  return `M0 ${ry} A${rx} ${ry} 0 1 0 ${w} ${ry} A${rx} ${ry} 0 1 0 0 ${ry} Z`;
}
