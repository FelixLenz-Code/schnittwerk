import { open, save, ask } from "@tauri-apps/plugin-dialog";
import { api } from "./api";
import { editor, MATS } from "./editor.svelte";
import { boxTransform, uid } from "./geometry";
import type { ImageInfo, ImportedVector, Obj, PathObj } from "./types";

const IMAGE_EXT = ["png", "jpg", "jpeg", "webp", "bmp", "gif", "tif", "tiff"];
const VECTOR_EXT = ["svg", "dxf", "pdf"];
const PROJECT_EXT = "schnitt";

function ext(path: string) {
  return path.split(".").pop()?.toLowerCase() ?? "";
}

/** Places a w×h box so it fits on the mat (max 80 %) and is centered. */
function placeOnMat(w: number, h: number) {
  const maxW = editor.mat.w * 0.8, maxH = editor.mat.h * 0.8;
  const s = Math.min(1, maxW / w, maxH / h);
  w *= s;
  h *= s;
  return { x: (editor.mat.w - w) / 2, y: (editor.mat.h - h) / 2, w, h };
}

function addImage(info: ImageInfo) {
  editor.images[info.id] = { src: info.src, name: info.name, pxW: info.width, pxH: info.height };
  // Assume 150 dpi for the initial size, then fit onto the mat.
  const mm = 25.4 / 150;
  const box = placeOnMat(info.width * mm, info.height * mm);
  editor.add([
    {
      id: uid(), type: "image", name: info.name, imageId: info.id, opacity: 1,
      ...box, rot: 0, flipX: false, flipY: false, visible: true, locked: false,
    },
  ]);
}

function addVector(v: ImportedVector) {
  const [bx, by, bw, bh] = v.bbox;
  // Keep the SVG's real size; only move it onto the mat if it lies outside.
  const onMat = bx >= 0 && by >= 0 && bx + bw <= editor.mat.w && by + bh <= editor.mat.h;
  const box = onMat ? { x: bx, y: by, w: bw, h: bh } : placeOnMat(bw, bh);
  const obj: PathObj = {
    id: uid(), type: "path", name: v.name, d: v.d, bbox: v.bbox, color: "#e5484d", fill: false,
    ...box, rot: 0, flipX: false, flipY: false, visible: true, locked: false,
  };
  editor.add([obj]);
  editor.rightTab = "object";
}

// ---------- clipboard ----------

let copied: Obj[] = [];
let pasteCount = 0;

export async function copySelection(cut = false) {
  const ids = editor.expandGroups(editor.selection);
  const objs = editor.objects.filter((o) => ids.includes(o.id));
  if (!objs.length) return;
  copied = $state.snapshot(objs) as Obj[];
  pasteCount = 0;
  if (cut) {
    editor.selection = ids;
    editor.removeSelected();
  }
  // So the next paste takes these objects, not an older picture.
  await api.clipboardMark().catch(() => {});
  editor.toast(`${objs.length} ${objs.length === 1 ? "Objekt" : "Objekte"} ${cut ? "ausgeschnitten" : "kopiert"}`);
}

export async function paste() {
  try {
    const c = await api.clipboardRead();
    if (c.kind === "image") {
      addImage(c.info);
      editor.rightTab = "trace";
      return;
    }
    if (c.kind === "svg") {
      addVector({ ...c.vector, name: "Eingefügt" });
      return;
    }
  } catch (e) {
    if (!copied.length) {
      editor.error(e);
      return;
    }
  }
  if (!copied.length) {
    editor.toast("Zwischenablage enthält kein Bild und keine Objekte.");
    return;
  }
  pasteCount++;
  const groupMap = new Map<string, string>();
  const objs = copied.map((o) => {
    const c = { ...structuredClone(o), id: uid(), x: o.x + 5 * pasteCount, y: o.y + 5 * pasteCount, locked: false };
    if (c.group) c.group = groupMap.get(c.group) ?? groupMap.set(c.group, uid()).get(c.group);
    return c;
  });
  editor.add(objs);
}

export async function importPaths(paths: string[]) {
  for (const path of paths) {
    try {
      const e = ext(path);
      if (IMAGE_EXT.includes(e)) {
        addImage(await api.loadImage(path));
        editor.rightTab = "trace";
      } else if (e === "svg") {
        addVector(await api.importSvg(path));
      } else if (e === "dxf") {
        addVector(await api.importDxf(path));
      } else if (e === "pdf") {
        const r = await api.importPdf(path);
        if (r.kind === "vector") addVector(r.vector);
        else {
          addImage(r.info);
          editor.rightTab = "trace";
          editor.toast("PDF ohne Vektoren – Seite als Bild eingefügt, zum Tracen bereit.");
        }
      } else if (e === PROJECT_EXT) {
        await openProject(path);
      } else {
        editor.toast(`Dateityp .${e} wird (noch) nicht unterstützt.`, "error");
      }
    } catch (err) {
      editor.error(err);
    }
  }
}

export async function importDialog() {
  const res = await open({
    multiple: true,
    filters: [
      { name: "Bilder & Vektoren", extensions: [...IMAGE_EXT, ...VECTOR_EXT] },
      { name: "Bilder", extensions: IMAGE_EXT },
      { name: "Vektoren (SVG, DXF, PDF)", extensions: VECTOR_EXT },
    ],
  });
  if (res) await importPaths(Array.isArray(res) ? res : [res]);
}

// ---------- project files ----------

interface ProjectFile {
  app: "schnittwerk";
  version: 1;
  mat: { name: string; w: number; h: number };
  printCut?: { enabled: boolean; page: "A4" | "Letter"; insetX?: number; insetY?: number; inset?: number };
  objects: Obj[];
  images: Record<string, { src: string; name: string }>;
}

export async function confirmDiscard(): Promise<boolean> {
  if (!editor.dirty || !editor.objects.length) return true;
  return ask("Ungespeicherte Änderungen gehen verloren. Fortfahren?", { title: "Schnittwerk", kind: "warning" });
}

export async function newProject() {
  if (!(await confirmDiscard())) return;
  editor.objects = [];
  editor.images = {};
  if (editor.printCut.enabled) editor.setPrintCut(false);
  editor.selection = [];
  editor.tracePreview = null;
  editor.filePath = null;
  editor.dirty = false;
  editor.resetHistory();
}

export async function saveProject(saveAs = false) {
  let path = editor.filePath;
  if (!path || saveAs) {
    path = await save({ defaultPath: "Projekt.schnitt", filters: [{ name: "Schnittwerk-Projekt", extensions: [PROJECT_EXT] }] });
    if (!path) return;
    if (!path.endsWith("." + PROJECT_EXT)) path += "." + PROJECT_EXT;
  }
  const used = new Set(editor.objects.flatMap((o) => (o.type === "image" ? [o.imageId] : [])));
  const images: ProjectFile["images"] = {};
  for (const id of used) {
    const a = editor.images[id];
    if (a) images[id] = { src: a.src, name: a.name };
  }
  const file: ProjectFile = {
    app: "schnittwerk",
    version: 1,
    mat: $state.snapshot(editor.mat),
    printCut: $state.snapshot(editor.printCut),
    objects: $state.snapshot(editor.objects) as Obj[],
    images,
  };
  try {
    await api.writeText(path, JSON.stringify(file));
    editor.filePath = path;
    editor.dirty = false;
    editor.toast("Projekt gespeichert", "ok");
  } catch (e) {
    editor.error(e);
  }
}

export async function openDialog() {
  if (!(await confirmDiscard())) return;
  const path = await open({ filters: [{ name: "Schnittwerk-Projekt", extensions: [PROJECT_EXT] }] });
  if (typeof path === "string") await openProject(path);
}

async function openProject(path: string) {
  const file = JSON.parse(await api.readText(path)) as ProjectFile;
  if (file.app !== "schnittwerk") throw new Error("Keine Schnittwerk-Projektdatei.");
  const remap: Record<string, string> = {};
  const images: typeof editor.images = {};
  for (const [oldId, img] of Object.entries(file.images)) {
    const info = await api.loadImageData(img.src, img.name);
    remap[oldId] = info.id;
    images[info.id] = { src: info.src, name: info.name, pxW: info.width, pxH: info.height };
  }
  editor.images = images;
  editor.objects = file.objects.map((o) => (o.type === "image" ? { ...o, imageId: remap[o.imageId] ?? o.imageId } : o));
  editor.mat = MATS.find((m) => m.name === file.mat.name) ?? file.mat;
  const pc = file.printCut;
  editor.printCut = pc
    ? { enabled: pc.enabled, page: pc.page, insetX: pc.insetX ?? pc.inset ?? 15, insetY: pc.insetY ?? pc.inset ?? 10 }
    : { enabled: false, page: "A4", insetX: 15, insetY: 10 };
  editor.selection = [];
  editor.tracePreview = null;
  editor.filePath = path;
  editor.dirty = false;
  editor.resetHistory();
  editor.fitMat();
}

// ---------- export ----------

export function buildSvg(onlyPaths = true): string {
  const { w, h } = editor.mat;
  const parts = [
    `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="${w}mm" height="${h}mm" viewBox="0 0 ${w} ${h}">`,
  ];
  for (const o of editor.objects) {
    if (!o.visible) continue;
    if (o.type === "path") {
      const [bx, by, bw, bh] = o.bbox;
      const t = `${boxTransform(o)} scale(${o.w / (bw || 1)} ${o.h / (bh || 1)}) translate(${-bx} ${-by})`;
      parts.push(`  <path d="${o.d}" transform="${t}" fill="none" stroke="${o.color}" stroke-width="0.25" fill-rule="evenodd"/>`);
    } else if (!onlyPaths) {
      const a = editor.images[o.imageId];
      if (a) parts.push(`  <image href="${a.src}" x="0" y="0" width="${o.w}" height="${o.h}" transform="${boxTransform(o)}" preserveAspectRatio="none"/>`);
    }
  }
  parts.push("</svg>");
  return parts.join("\n");
}

export async function exportSvg() {
  if (!editor.objects.some((o) => o.type === "path")) {
    editor.toast("Keine Schnittlinien zum Exportieren.", "error");
    return;
  }
  let path = await save({ defaultPath: "Schnittlinien.svg", filters: [{ name: "SVG", extensions: ["svg"] }] });
  if (!path) return;
  if (!path.toLowerCase().endsWith(".svg")) path += ".svg";
  try {
    await api.writeText(path, buildSvg(true));
    editor.toast("SVG exportiert", "ok");
  } catch (e) {
    editor.error(e);
  }
}
