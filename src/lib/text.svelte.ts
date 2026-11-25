import { api } from "./api";
import { editor } from "./editor.svelte";
import { uid } from "./geometry";
import { parsePath, serializePath } from "./pathdata";
import { pathMatrix } from "./plotter.svelte";
import type { PathObj, TextSpec, Vec2 } from "./types";

class FontStore {
  families = $state<string[]>([]);
  #loading: Promise<void> | null = null;

  load() {
    this.#loading ??= api
      .fontFamilies()
      .then((f) => {
        this.families = f;
      })
      .catch((e) => editor.error(e));
    return this.#loading;
  }

  get fallback(): string {
    const prefer = ["Noto Sans", "DejaVu Sans", "Cantarell", "Liberation Sans", "Ubuntu"];
    return prefer.find((p) => this.families.includes(p)) ?? this.families[0] ?? "sans-serif";
  }
}

export const fonts = new FontStore();

let lastSpec: Omit<TextSpec, "content"> | null = null;

/** Circle of radius r around (0, 0) for text over the top (clockwise from the
 *  bottom) or along the bottom (counter-clockwise from the top), centred. */
export function arcPath(kind: "arcTop" | "arcBottom", r: number): string {
  return kind === "arcTop"
    ? `M0 ${r}A${r} ${r} 0 1 1 0 ${-r}A${r} ${r} 0 1 1 0 ${r}Z`
    : `M0 ${-r}A${r} ${r} 0 1 0 0 ${r}A${r} ${r} 0 1 0 0 ${-r}Z`;
}

/** Path data of a path object in mat millimetres (text on that path). */
export function worldPathD(o: PathObj): string {
  const [a, b, c, d, e, f] = pathMatrix(o, null);
  const tr = (p: Vec2) => ({ x: a * p.x + c * p.y + e, y: b * p.x + d * p.y + f });
  const subs = parsePath(o.d).map((s) => ({
    closed: s.closed,
    nodes: s.nodes.map((n) => ({ ...tr(n), cin: n.cin && tr(n.cin), cout: n.cout && tr(n.cout) })),
  }));
  return serializePath(subs);
}

async function outline(spec: TextSpec): Promise<{ d: string; bbox: [number, number, number, number] }> {
  const curve = spec.curve;
  if (curve && curve.kind !== "custom") {
    // Arcs are centred on the top / bottom of the circle.
    spec = { ...spec, path: arcPath(curve.kind, Math.max(1, curve.radius)), align: "middle" };
  } else if (!curve) {
    spec = { ...spec, path: null };
  }
  const v = await api.textToPath(spec);
  if (!spec.weld) return v;
  const w = await api.offsetPaths([{ d: v.d, matrix: [1, 0, 0, 1, 0, 0] }], 0, 20);
  return { d: w.d, bbox: w.bbox };
}

export async function createText(at: Vec2) {
  await fonts.load();
  const spec: TextSpec = lastSpec
    ? { ...lastSpec, content: "Text" }
    : { content: "Text", family: fonts.fallback, size: 20, bold: false, italic: false, spacing: 0, lineHeight: 1.2, align: "start", weld: false };
  try {
    const o = await outline(spec);
    const obj: PathObj = {
      id: uid(), type: "path", name: "Text", d: o.d, bbox: o.bbox, color: "#e5484d", fill: true,
      x: at.x, y: at.y, w: o.bbox[2], h: o.bbox[3], rot: 0, flipX: false, flipY: false, visible: true, locked: false,
      text: spec,
    };
    editor.add([obj]);
    editor.rightTab = "object";
  } catch (e) {
    editor.error(e);
  }
}

const pending = new Map<string, number>();
const seqs = new Map<string, number>();

/**
 * Re-renders a text object after its spec changed (debounced, keeps scaling).
 * place = "exact": put the outlines 1:1 where they lie (text on a picked path);
 * "center": keep the box centre (switching between straight and arc);
 * default: text along a path stays anchored to the path, otherwise the
 * top-left corner stays.
 */
export function regenerate(id: string, place?: "exact" | "center") {
  clearTimeout(pending.get(id));
  pending.set(
    id,
    window.setTimeout(async () => {
      const o = editor.byId(id);
      if (!o || o.type !== "path" || !o.text) return;
      const my = (seqs.get(id) ?? 0) + 1;
      seqs.set(id, my);
      const spec = $state.snapshot(o.text) as TextSpec;
      try {
        const r = await outline(spec);
        const cur = editor.byId(id);
        if (!cur || cur.type !== "path" || seqs.get(id) !== my) return;
        const sx = cur.w / (cur.bbox[2] || 1), sy = cur.h / (cur.bbox[3] || 1);
        const old = { x: cur.x, y: cur.y, w: cur.w, h: cur.h, bbox: cur.bbox };
        cur.d = r.d;
        cur.bbox = r.bbox;
        if (place === "exact") {
          [cur.x, cur.y, cur.w, cur.h] = r.bbox;
        } else {
          cur.w = r.bbox[2] * sx;
          cur.h = r.bbox[3] * sy;
          if (place === "center") {
            cur.x = old.x + old.w / 2 - cur.w / 2;
            cur.y = old.y + old.h / 2 - cur.h / 2;
          } else if (spec.curve && cur.rot % 360 === 0 && !cur.flipX && !cur.flipY) {
            // Keep the glyphs where they are on the path.
            cur.x = old.x + (r.bbox[0] - old.bbox[0]) * sx;
            cur.y = old.y + (r.bbox[1] - old.bbox[1]) * sy;
          }
        }
        cur.name = spec.content.split("\n")[0].slice(0, 30) || "Text";
        const { content: _, ...rest } = spec;
        lastSpec = rest;
      } catch (e) {
        editor.error(e);
      }
    }, place ? 0 : 200),
  );
}
