import { api } from "./api";
import { editor } from "./editor.svelte";
import { decimalComma, t } from "./i18n.svelte";
import { uid } from "./geometry";
import { perspective } from "./perspective.svelte";
import type { ImageObj, PathObj } from "./types";

const KEY = "schnittwerk.pixscan";
/** Inner frame line of the mat (mm): across × along the feed. */
export const FRAME_W = 195;
export const FRAME_H = 295;
/** Test crosses for calibration, relative to the frame (mm). */
const CROSSES: [number, number][] = [[20, 20], [175, 20], [20, 275], [175, 275], [97.5, 147.5]];
const CROSS = 10;

interface Offset {
  /** Position of the frame's top-left corner on the mat (mm). */
  x: number;
  y: number;
}

function load(): Offset {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "");
    if (typeof v?.x === "number" && typeof v?.y === "number") return v;
  } catch {}
  // Frame centred in the 8″ × 12″ cutting area until calibrated.
  return { x: 4, y: 5 };
}

/** PixScan mat: straighten a photo of the mat and place it where the
 *  plotter will cut, so cut lines can be drawn right on the material. */
class PixScanStore {
  offset = $state<Offset>(load());
  busy = $state(false);
  /** Original photo and frame per placed object, for turning it round. */
  #sources = new Map<string, { imageId: string; frame: [number, number][]; name: string }>();

  save() {
    try {
      localStorage.setItem(KEY, JSON.stringify(this.offset));
    } catch {}
  }

  /** Finds the frame automatically; otherwise lets the user set the corners. */
  async align(img: ImageObj) {
    const a = editor.images[img.imageId];
    if (!a) return;
    this.busy = true;
    try {
      const det = await api.detectPixscan(img.imageId);
      if (!det) {
        editor.toast("Rahmen nicht gefunden – bitte die vier Ecken setzen", "info");
        await perspective.start(img, (pts) => this.place(img.id, img.imageId, pts, a.name));
        return;
      }
      await this.place(img.id, img.imageId, det.frame, a.name);
      if (!det.oriented) editor.toast("Großes Quadrat nicht erkannt – falls verdreht: „Um 180° drehen“", "info");
      else editor.toast("PixScan-Matte erkannt", "ok");
    } catch (e) {
      editor.error(e);
    } finally {
      this.busy = false;
    }
  }

  /** Lets the user correct the detected corners by hand. */
  async editCorners(img: ImageObj) {
    const src = this.#sources.get(img.id);
    if (!src) return;
    // Corner tool runs on the original photo; switch back to it meanwhile.
    const live = editor.byId(img.id);
    if (live?.type !== "image") return;
    const a = editor.images[src.imageId];
    if (!a) return;
    editor.checkpoint("PixScan");
    live.imageId = src.imageId;
    live.h = (live.w * a.pxH) / a.pxW;
    await perspective.start(live, (pts) => this.place(img.id, src.imageId, pts, src.name), src.frame);
  }

  /** Straightens the photo and puts it at the frame's place on the mat. */
  async place(objId: string, imageId: string, frame: [number, number][], name: string) {
    const r = await api.pixscanRectify(imageId, frame, name);
    editor.images[r.info.id] = { src: r.info.src, name, pxW: r.info.width, pxH: r.info.height };
    const live = editor.byId(objId);
    if (live?.type !== "image") return;
    editor.checkpoint("PixScan");
    if (editor.printCut.enabled) editor.setPrintCut(false);
    const x = this.offset.x - r.margin;
    const y = this.offset.y - r.margin;
    Object.assign(live, {
      imageId: r.info.id,
      name: "PixScan-Foto",
      x,
      y,
      w: r.frameW + 2 * r.margin,
      h: r.frameH + 2 * r.margin,
      rot: 0,
      flipX: false,
      flipY: false,
      locked: true,
      pixscan: { x, y },
    });
    this.#sources.set(objId, { imageId, frame, name });
  }

  canEdit(img: ImageObj) {
    return this.#sources.has(img.id);
  }

  /** The mat was the other way round: corner 1 is diagonally opposite. */
  async turn(img: ImageObj) {
    const src = this.#sources.get(img.id);
    if (!src) return;
    const f = src.frame;
    this.busy = true;
    try {
      await this.place(img.id, src.imageId, [f[2], f[3], f[0], f[1]], src.name);
    } catch (e) {
      editor.error(e);
    } finally {
      this.busy = false;
    }
  }

  // ---------- calibration ----------

  /** Crosses to draw with the pen (or cut) on paper on the PixScan mat. */
  addTestCrosses() {
    const h = CROSS / 2;
    const d = `M0 ${h}H${CROSS}M${h} 0V${CROSS}`;
    const group = uid();
    const objs: PathObj[] = CROSSES.map(([cx, cy]) => ({
      id: uid(), type: "path", name: "PixScan-Testkreuz", d, bbox: [0, 0, CROSS, CROSS], color: "#30a46c", fill: false,
      x: this.offset.x + cx - h, y: this.offset.y + cy - h, w: CROSS, h: CROSS,
      rot: 0, flipX: false, flipY: false, visible: true, locked: false, group,
    }));
    editor.add(objs);
    editor.toast("Testkreuze angelegt – mit dem Stift zeichnen", "ok");
  }

  /** Moves the placed photo (calibration: line up drawn and planned crosses). */
  nudge(img: ImageObj, dx: number, dy: number) {
    const live = editor.byId(img.id);
    if (live?.type !== "image") return;
    editor.checkpoint("PixScan");
    live.x += dx;
    live.y += dy;
  }

  /** How far the photo was moved since it was placed. */
  shift(img: ImageObj): { dx: number; dy: number } | null {
    if (!img.pixscan) return null;
    return { dx: img.x - img.pixscan.x, dy: img.y - img.pixscan.y };
  }

  /** The photo now lies right: its shift becomes part of the offset. */
  applyCalibration(img: ImageObj) {
    const s = this.shift(img);
    const live = editor.byId(img.id);
    if (!s || live?.type !== "image") return;
    this.offset = { x: round(this.offset.x + s.dx), y: round(this.offset.y + s.dy) };
    this.save();
    live.pixscan = { x: live.x, y: live.y };
    editor.toast(t("Kalibrierung gespeichert (X {x} mm, Y {y} mm)", { x: fmt(this.offset.x), y: fmt(this.offset.y) }), "ok");
  }
}

const round = (v: number) => Math.round(v * 100) / 100;
const fmt = (v: number) => (decimalComma() ? v.toFixed(2).replace(".", ",") : v.toFixed(2));

export const pixscan = new PixScanStore();
