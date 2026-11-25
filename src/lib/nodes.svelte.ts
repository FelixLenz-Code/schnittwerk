// Node editing of a path object: nodes and Bézier handles can be moved,
// inserted, deleted and switched between corner / smooth, line / curve.

import { editor } from "./editor.svelte";
import { fromLocal, rotate, toLocal } from "./geometry";
import { parsePath, pathBbox, serializePath, splitSegment, type PNode, type Sub } from "./pathdata";
import type { PathObj, Vec2 } from "./types";

/** "si:ni" – subpath and node index. */
export type NodeKey = string;
export const key = (si: number, ni: number): NodeKey => `${si}:${ni}`;
const unkey = (k: NodeKey) => k.split(":").map(Number) as [number, number];

/** More nodes than this: editing a traced path would be unusable. */
export const NODE_LIMIT = 4000;

class NodeEditor {
  objId = $state<string | null>(null);
  subs = $state<Sub[]>([]);
  sel = $state<NodeKey[]>([]);
  /** Path data last written by the editor (detects undo / outside changes). */
  #lastD = "";

  get obj(): PathObj | null {
    const o = this.objId ? editor.byId(this.objId) : undefined;
    return o?.type === "path" ? o : null;
  }

  get count(): number {
    return this.subs.reduce((n, s) => n + s.nodes.length, 0);
  }

  start(o: PathObj) {
    if (o.locked) return;
    let subs: Sub[];
    try {
      subs = parsePath(o.d);
    } catch (e) {
      editor.error(e);
      return;
    }
    const n = subs.reduce((k, s) => k + s.nodes.length, 0);
    if (n > NODE_LIMIT) {
      editor.toast(`Pfad hat ${n} Knoten – zum Bearbeiten erst mit stärkerer Glättung tracen.`, "error");
      return;
    }
    // Nodes whose handles are already collinear count as smooth.
    for (const s of subs) for (const p of s.nodes) p.smooth = isSmooth(p);
    if (o.text) {
      // Edited outlines can no longer be regenerated from the text.
      editor.checkpoint();
      delete o.text;
      editor.toast("Text wird beim Bearbeiten der Knoten zur Form.");
    }
    if (o.shape) {
      // Same for shapes: the nodes are edited directly from now on.
      editor.checkpoint();
      delete o.shape;
    }
    editor.selection = [o.id];
    this.#lastD = o.d;
    this.subs = subs;
    this.sel = [];
    this.objId = o.id;
  }

  /** Re-reads the path after undo/redo; ends editing if the object is gone. */
  sync() {
    if (!this.objId) return;
    const o = this.obj;
    if (!o || !o.visible || o.locked) return this.stop();
    if (o.d === this.#lastD) return;
    try {
      const subs = parsePath(o.d);
      for (const s of subs) for (const p of s.nodes) p.smooth = isSmooth(p);
      this.subs = subs;
      this.sel = [];
      this.#lastD = o.d;
    } catch {
      this.stop();
    }
  }

  stop() {
    this.objId = null;
    this.subs = [];
    this.sel = [];
  }

  // ---------- coordinates ----------

  /** Path units → world (mm on the mat). */
  toWorld(p: Vec2, o = this.obj!): Vec2 {
    const [bx, by, bw, bh] = o.bbox;
    return fromLocal(o, { x: (p.x - bx) * (o.w / (bw || 1)), y: (p.y - by) * (o.h / (bh || 1)) });
  }

  /** World → path units. */
  fromWorld(p: Vec2, o = this.obj!): Vec2 {
    const [bx, by, bw, bh] = o.bbox;
    const l = toLocal(o, p);
    return { x: l.x / (o.w / (bw || 1)) + bx, y: l.y / (o.h / (bh || 1)) + by };
  }

  node(k: NodeKey): PNode | undefined {
    const [si, ni] = unkey(k);
    return this.subs[si]?.nodes[ni];
  }

  /** Writes the edited nodes back into the object; nodes that did not
   *  move stay exactly where they were on the mat. */
  write() {
    const o = this.obj;
    if (!o) return;
    const [bx, by, bw, bh] = o.bbox;
    const sx = o.w / (bw || 1), sy = o.h / (bh || 1);
    const nb = pathBbox(this.subs);
    // World position of the new bbox corner under the old mapping.
    const W = this.toWorld({ x: nb[0], y: nb[1] }, o);
    const w = Math.max(nb[2] * sx, 0.01), h = Math.max(nb[3] * sy, 0.01);
    let ox = -w / 2, oy = -h / 2;
    if (o.flipX) ox = -ox;
    if (o.flipY) oy = -oy;
    const off = rotate({ x: ox, y: oy }, o.rot);
    const c = { x: W.x - off.x, y: W.y - off.y };
    o.d = serializePath(this.subs);
    this.#lastD = o.d;
    o.bbox = [nb[0], nb[1], nb[2] || 0.01, nb[3] || 0.01];
    o.w = w;
    o.h = h;
    o.x = c.x - w / 2;
    o.y = c.y - h / 2;
  }

  // ---------- editing ----------

  /** Moves the selected nodes (with their handles) by a delta in path units. */
  moveSelected(orig: Map<NodeKey, PNode>, dx: number, dy: number) {
    for (const [k, o] of orig) {
      const p = this.node(k);
      if (!p) continue;
      p.x = o.x + dx;
      p.y = o.y + dy;
      if (o.cin) p.cin = { x: o.cin.x + dx, y: o.cin.y + dy };
      if (o.cout) p.cout = { x: o.cout.x + dx, y: o.cout.y + dy };
    }
    this.write();
  }

  /** Nudges the selected nodes by a world delta (mm, arrow keys). */
  nudge(dx: number, dy: number) {
    const o = this.obj;
    if (!o || !this.sel.length) return;
    editor.checkpoint();
    const a = this.fromWorld({ x: 0, y: 0 }, o), b = this.fromWorld({ x: dx, y: dy }, o);
    const orig = new Map(this.sel.map((k) => [k, $state.snapshot(this.node(k)!) as PNode]));
    this.moveSelected(orig, b.x - a.x, b.y - a.y);
  }

  selectAll() {
    this.sel = this.subs.flatMap((s, si) => s.nodes.map((_, ni) => key(si, ni)));
  }

  /** Moves one handle; on smooth nodes the opposite handle follows. */
  moveHandle(k: NodeKey, side: "cin" | "cout", to: Vec2) {
    const p = this.node(k);
    if (!p) return;
    p[side] = to;
    const other = side === "cin" ? "cout" : "cin";
    const o = p[other];
    if (p.smooth && o) {
      const len = Math.hypot(o.x - p.x, o.y - p.y);
      const dx = p.x - to.x, dy = p.y - to.y;
      const d = Math.hypot(dx, dy) || 1;
      p[other] = { x: p.x + (dx / d) * len, y: p.y + (dy / d) * len };
    }
    this.write();
  }

  insertAt(si: number, i: number, t: number) {
    const s = this.subs[si];
    if (!s) return;
    editor.checkpoint();
    const ni = splitSegment(s, i, t);
    // Indices behind the new node shift by one.
    this.sel = [key(si, ni)];
    this.write();
  }

  deleteSelected() {
    if (!this.sel.length) return;
    editor.checkpoint();
    const bySub = new Map<number, number[]>();
    for (const k of this.sel) {
      const [si, ni] = unkey(k);
      bySub.set(si, [...(bySub.get(si) ?? []), ni]);
    }
    for (const [si, list] of bySub) {
      const s = this.subs[si];
      for (const ni of list.sort((a, b) => b - a)) s.nodes.splice(ni, 1);
    }
    // Subpaths that are too short to cut disappear.
    this.subs = this.subs.filter((s) => s.nodes.length >= 2);
    this.sel = [];
    if (!this.subs.length) {
      const id = this.objId!;
      this.stop();
      editor.selection = [id];
      editor.removeSelected();
      return;
    }
    this.write();
  }

  /** Corner ↔ smooth for the selected nodes. Smooth nodes get handles. */
  setSmooth(smooth: boolean) {
    if (!this.sel.length) return;
    editor.checkpoint();
    for (const k of this.sel) {
      const [si, ni] = unkey(k);
      const s = this.subs[si];
      const p = s.nodes[ni];
      p.smooth = smooth;
      if (!smooth) continue;
      const n = s.nodes.length;
      const prev = s.closed || ni > 0 ? s.nodes[(ni - 1 + n) % n] : null;
      const next = s.closed || ni < n - 1 ? s.nodes[(ni + 1) % n] : null;
      // Tangent from neighbour to neighbour, handles a third of the way.
      const a = prev ?? p, b = next ?? p;
      let tx = b.x - a.x, ty = b.y - a.y;
      const tl = Math.hypot(tx, ty) || 1;
      tx /= tl;
      ty /= tl;
      if (prev) {
        const l = Math.hypot(p.x - prev.x, p.y - prev.y) / 3;
        p.cin = { x: p.x - tx * l, y: p.y - ty * l };
      }
      if (next) {
        const l = Math.hypot(next.x - p.x, next.y - p.y) / 3;
        p.cout = { x: p.x + tx * l, y: p.y + ty * l };
      }
    }
    this.write();
  }

  /** Segments after the selected nodes become straight lines (true) or curves. */
  setStraight(straight: boolean) {
    if (!this.sel.length) return;
    editor.checkpoint();
    for (const k of this.sel) {
      const [si, ni] = unkey(k);
      const s = this.subs[si];
      const n = s.nodes.length;
      if (!s.closed && ni === n - 1) continue;
      const a = s.nodes[ni], b = s.nodes[(ni + 1) % n];
      if (straight) {
        delete a.cout;
        delete b.cin;
        a.smooth = b.smooth = false;
      } else if (!a.cout && !b.cin) {
        a.cout = { x: a.x + (b.x - a.x) / 3, y: a.y + (b.y - a.y) / 3 };
        b.cin = { x: a.x + ((b.x - a.x) * 2) / 3, y: a.y + ((b.y - a.y) * 2) / 3 };
      }
    }
    this.write();
  }
}

function isSmooth(p: PNode): boolean {
  if (!p.cin || !p.cout) return false;
  const ax = p.x - p.cin.x, ay = p.y - p.cin.y, bx = p.cout.x - p.x, by = p.cout.y - p.y;
  const la = Math.hypot(ax, ay), lb = Math.hypot(bx, by);
  if (la < 1e-9 || lb < 1e-9) return false;
  return (ax * bx + ay * by) / (la * lb) > 0.995;
}

export const nodes = new NodeEditor();
