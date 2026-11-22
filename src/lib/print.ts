import { editor, regBracketPath, REG_LINE_WIDTH } from "./editor.svelte";
import type { Obj } from "./types";

const imageCache = new Map<string, HTMLImageElement>();

export function loadImage(src: string): Promise<HTMLImageElement> {
  const hit = imageCache.get(src);
  if (hit?.complete) return Promise.resolve(hit);
  return new Promise((resolve, reject) => {
    const img = hit ?? new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("Bild konnte nicht geladen werden"));
    if (!hit) {
      img.src = src;
      imageCache.set(src, img);
    }
  });
}

function applyBox(ctx: CanvasRenderingContext2D, o: Obj) {
  ctx.translate(o.x + o.w / 2, o.y + o.h / 2);
  ctx.rotate((o.rot * Math.PI) / 180);
  ctx.scale(o.flipX ? -1 : 1, o.flipY ? -1 : 1);
  ctx.translate(-o.w / 2, -o.h / 2);
}

/**
 * Renders the printable layer of the page (images, printed fills and the
 * registration marks – no cut lines) as a PNG data URL.
 */
export async function renderPage(dpi = 300): Promise<string> {
  const k = dpi / 25.4;
  const { w, h } = editor.mat;
  const canvas = document.createElement("canvas");
  canvas.width = Math.round(w * k);
  canvas.height = Math.round(h * k);
  const ctx = canvas.getContext("2d")!;
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = "high";
  ctx.fillStyle = "#fff";
  ctx.fillRect(0, 0, canvas.width, canvas.height);

  for (const o of editor.objects) {
    if (!o.visible) continue;
    ctx.save();
    ctx.setTransform(k, 0, 0, k, 0, 0);
    applyBox(ctx, o);
    if (o.type === "image") {
      const a = editor.images[o.imageId];
      if (a) {
        ctx.globalAlpha = o.opacity;
        ctx.drawImage(await loadImage(a.src), 0, 0, o.w, o.h);
      }
    } else if (o.printFill) {
      const [bx, by, bw, bh] = o.bbox;
      ctx.scale(o.w / (bw || 1), o.h / (bh || 1));
      ctx.translate(-bx, -by);
      ctx.fillStyle = o.printFill;
      ctx.fill(new Path2D(o.d), "evenodd");
    }
    ctx.restore();
  }

  const r = editor.regmarks;
  if (r) {
    ctx.save();
    ctx.setTransform(k, 0, 0, k, 0, 0);
    // Keep the mark zones clean even if an image overlaps them.
    ctx.fillStyle = "#fff";
    for (const [x, y, zw, zh] of editor.regZones()) ctx.fillRect(x, y, zw, zh);
    ctx.fillStyle = "#000";
    ctx.fillRect(r.originX, r.originY, 5, 5);
    ctx.strokeStyle = "#000";
    ctx.lineWidth = REG_LINE_WIDTH;
    ctx.lineCap = "butt";
    ctx.lineJoin = "miter";
    ctx.stroke(new Path2D(regBracketPath(r)));
    ctx.restore();
  }
  return canvas.toDataURL("image/png");
}
