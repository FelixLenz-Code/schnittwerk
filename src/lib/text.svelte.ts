import { api } from "./api";
import { editor } from "./editor.svelte";
import { uid } from "./geometry";
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

async function outline(spec: TextSpec): Promise<{ d: string; bbox: [number, number, number, number] }> {
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

/** Re-renders a text object after its spec changed (debounced, keeps scaling). */
export function regenerate(id: string) {
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
        cur.d = r.d;
        cur.bbox = r.bbox;
        cur.w = r.bbox[2] * sx;
        cur.h = r.bbox[3] * sy;
        cur.name = spec.content.split("\n")[0].slice(0, 30) || "Text";
        const { content: _, ...rest } = spec;
        lastSpec = rest;
      } catch (e) {
        editor.error(e);
      }
    }, 200),
  );
}
