import { api } from "./api";
import { editor } from "./editor.svelte";
import { loadImage } from "./print";
import type { ImageObj, Vec2 } from "./types";

export type BrushMode = "erase" | "paint";

class BrushStore {
  mode = $state<BrushMode | null>(null);
  /** Brush diameter in mm on the mat. */
  size = $state(3);
  color = $state("#000000");
  /** Stroke being drawn, in source-image pixels. */
  stroke = $state<Vec2[]>([]);
  busy = $state(false);

  toggle(mode: BrushMode) {
    this.mode = this.mode === mode ? null : mode;
  }

  /** Brush diameter in image pixels for an image object. */
  sizePx(img: ImageObj): number {
    const a = editor.images[img.imageId];
    return a ? (this.size * a.pxW) / img.w : 1;
  }

  /** Burns the current stroke into a new version of the image (undoable). */
  async commit(img: ImageObj) {
    const pts = this.stroke;
    const a = editor.images[img.imageId];
    if (!a || !pts.length || !this.mode) {
      this.stroke = [];
      return;
    }
    this.busy = true;
    try {
      const el = await loadImage(a.src);
      const canvas = document.createElement("canvas");
      canvas.width = a.pxW;
      canvas.height = a.pxH;
      const ctx = canvas.getContext("2d")!;
      ctx.drawImage(el, 0, 0, a.pxW, a.pxH);
      ctx.globalCompositeOperation = this.mode === "erase" ? "destination-out" : "source-over";
      ctx.strokeStyle = ctx.fillStyle = this.mode === "erase" ? "#000" : this.color;
      ctx.lineWidth = this.sizePx(img);
      ctx.lineCap = "round";
      ctx.lineJoin = "round";
      if (pts.length === 1) {
        ctx.beginPath();
        ctx.arc(pts[0].x, pts[0].y, ctx.lineWidth / 2, 0, Math.PI * 2);
        ctx.fill();
      } else {
        ctx.beginPath();
        ctx.moveTo(pts[0].x, pts[0].y);
        for (const p of pts.slice(1)) ctx.lineTo(p.x, p.y);
        ctx.stroke();
      }
      const info = await api.loadImageData(canvas.toDataURL("image/png"), a.name);
      editor.images[info.id] = { src: info.src, name: a.name, pxW: info.width, pxH: info.height };
      const live = editor.byId(img.id);
      if (live?.type === "image") {
        editor.checkpoint("Bild bearbeiten");
        live.imageId = info.id;
      }
    } catch (e) {
      editor.error(e);
    } finally {
      this.stroke = [];
      this.busy = false;
    }
  }
}

export const brush = new BrushStore();
