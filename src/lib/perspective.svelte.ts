import { api } from "./api";
import { brush } from "./brush.svelte";
import { editor } from "./editor.svelte";
import { center } from "./geometry";
import type { ImageObj, Vec2 } from "./types";

/** Paper sizes in mm (portrait); `null` = take the shape from the corners. */
export const PAPERS: Record<string, { label: string; size: [number, number] | null }> = {
  free: { label: "Aus den Ecken", size: null },
  a4: { label: "A4", size: [210, 297] },
  a5: { label: "A5", size: [148, 210] },
  a6: { label: "A6", size: [105, 148] },
  letter: { label: "Letter", size: [215.9, 279.4] },
  square: { label: "Quadratisch", size: null },
};

/** Perspective correction of a photographed sheet: four draggable corners
 *  on the image, then the quad is straightened into a rectangle. */
class PerspectiveStore {
  /** Image being corrected, null = tool inactive. */
  objId = $state<string | null>(null);
  /** Corners in source-image pixels: TL, TR, BR, BL. */
  pts = $state<Vec2[]>([]);
  paper = $state("free");
  busy = $state(false);
  /** The corners came from automatic page detection. */
  detected = $state(false);
  /** Set when the corners are for another tool (PixScan frame): called
   *  with the corners instead of straightening into a paper size. */
  custom = $state<((pts: [number, number][]) => Promise<void>) | null>(null);

  async start(img: ImageObj, custom: ((pts: [number, number][]) => Promise<void>) | null = null, initial: [number, number][] | null = null) {
    const a = editor.images[img.imageId];
    if (!a) return;
    brush.mode = null;
    editor.picking = null;
    this.custom = custom;
    this.busy = true;
    try {
      const q = initial ?? (custom ? null : await api.detectPage(img.imageId).catch(() => null));
      this.detected = !!q;
      const m = Math.min(a.pxW, a.pxH) * 0.06;
      this.pts = q
        ? q.map(([x, y]) => ({ x, y }))
        : [
            { x: m, y: m },
            { x: a.pxW - m, y: m },
            { x: a.pxW - m, y: a.pxH - m },
            { x: m, y: a.pxH - m },
          ];
      this.objId = img.id;
    } finally {
      this.busy = false;
    }
  }

  cancel() {
    this.objId = null;
    this.pts = [];
    this.custom = null;
  }

  /** Moves the corner numbering on by one (PixScan: which corner is 1). */
  rotate() {
    this.pts = [...this.pts.slice(1), this.pts[0]];
  }

  /** Width / height of the corrected image, from the paper choice. */
  #aspect(): number | null {
    const p = this.pts;
    const d = (a: Vec2, b: Vec2) => Math.hypot(a.x - b.x, a.y - b.y);
    const landscape = d(p[0], p[1]) + d(p[3], p[2]) > d(p[0], p[3]) + d(p[1], p[2]);
    if (this.paper === "square") return 1;
    const size = PAPERS[this.paper]?.size;
    if (!size) return null;
    const [w, h] = landscape ? [size[1], size[0]] : size;
    return w / h;
  }

  async apply() {
    const img = this.objId ? editor.byId(this.objId) : undefined;
    if (img?.type !== "image" || this.pts.length !== 4) return this.cancel();
    const a = editor.images[img.imageId];
    if (!a) return this.cancel();
    this.busy = true;
    if (this.custom) {
      try {
        await this.custom(this.pts.map((p) => [p.x, p.y]));
        this.cancel();
      } catch (e) {
        editor.error(e);
      } finally {
        this.busy = false;
      }
      return;
    }
    try {
      const info = await api.warpImage(img.imageId, this.pts.map((p) => [p.x, p.y]), this.#aspect(), a.name);
      editor.images[info.id] = { src: info.src, name: a.name, pxW: info.width, pxH: info.height };
      const live = editor.byId(img.id);
      if (live?.type === "image") {
        editor.checkpoint();
        const c = center(live);
        const size = PAPERS[this.paper]?.size;
        let w: number, h: number;
        if (size) {
          // Real paper size: the drawing comes out at its true scale.
          [w, h] = info.width > info.height ? [size[1], size[0]] : size;
        } else {
          w = live.w;
          h = (live.w * info.height) / info.width;
        }
        live.imageId = info.id;
        live.w = w;
        live.h = h;
        live.x = c.x - w / 2;
        live.y = c.y - h / 2;
      }
      editor.toast("Perspektive korrigiert", "ok");
      this.cancel();
    } catch (e) {
      editor.error(e);
    } finally {
      this.busy = false;
    }
  }
}

export const perspective = new PerspectiveStore();
