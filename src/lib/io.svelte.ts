import { open, save, ask, message } from "@tauri-apps/plugin-dialog";
import { api, inApp } from "./api";
import { editor, MATS, type Mat, type ProjectKind } from "./editor.svelte";
import { wizardUi } from "./wizard.svelte";
import { t } from "./i18n.svelte";
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

/** Panel for a newly imported picture: the sticker assistant for sticker
 *  projects (or a print & cut page), otherwise tracing. */
function imageTab(): "sticker" | "trace" {
  const k = editor.projectKind;
  if (k) return k === "sticker" ? "sticker" : "trace";
  return editor.printCut.enabled ? "sticker" : "trace";
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
  ], true, "Importieren");
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
  editor.add([obj], true, "Importieren");
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
  const n = objs.length;
  editor.toast(
    cut ? t(n === 1 ? "1 Objekt ausgeschnitten" : "{n} Objekte ausgeschnitten", { n }) : t(n === 1 ? "1 Objekt kopiert" : "{n} Objekte kopiert", { n }),
  );
}

export async function paste() {
  try {
    const c = await api.clipboardRead();
    if (c.kind === "image") {
      addImage(c.info);
      editor.rightTab = imageTab();
      return;
    }
    if (c.kind === "svg") {
      addVector({ ...c.vector, name: t("Eingefügt") });
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
  editor.add(objs, true, "Einfügen");
}

export async function importPaths(paths: string[]) {
  for (const path of paths) {
    try {
      const e = ext(path);
      if (IMAGE_EXT.includes(e)) {
        addImage(await api.loadImage(path));
        editor.rightTab = imageTab();
      } else if (e === "svg") {
        addVector(await api.importSvg(path));
      } else if (e === "dxf") {
        addVector(await api.importDxf(path));
      } else if (e === "pdf") {
        const r = await api.importPdf(path);
        if (r.kind === "vector") addVector(r.vector);
        else {
          addImage(r.info);
          editor.rightTab = imageTab();
          editor.toast("PDF ohne Vektoren – Seite als Bild eingefügt, zum Tracen bereit.");
        }
      } else if (e === PROJECT_EXT) {
        await openProject(path);
      } else {
        editor.toast(t("Dateityp .{ext} wird (noch) nicht unterstützt.", { ext: e }), "error");
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
      { name: t("Bilder & Vektoren"), extensions: [...IMAGE_EXT, ...VECTOR_EXT] },
      { name: t("Bilder"), extensions: IMAGE_EXT },
      { name: t("Vektoren (SVG, DXF, PDF)"), extensions: VECTOR_EXT },
    ],
  });
  if (res) await importPaths(Array.isArray(res) ? res : [res]);
}

// ---------- project files ----------

interface ProjectFile {
  app: "schnittwerk";
  version: 1;
  mat: Mat;
  printCut?: { enabled: boolean; page: "A4" | "Letter"; insetX?: number; insetY?: number; inset?: number };
  guides?: { v: number[]; h: number[] };
  material?: string | null;
  kind?: ProjectKind | null;
  objects: Obj[];
  /** Saved: name (the backend adds the picture data as `src`).
   *  Opened: the picture as registered by the backend. */
  images: Record<string, { name: string; id?: string; width?: number; height?: number; src?: string }>;
  /** Only in the recovery copy. */
  autosave?: { savedAt: number; filePath: string | null };
}

export async function confirmDiscard(): Promise<boolean> {
  if (!editor.dirty || !editor.objects.length) return true;
  return ask(t("Ungespeicherte Änderungen gehen verloren. Fortfahren?"), { title: "Schnittwerk", kind: "warning", okLabel: t("Fortfahren"), cancelLabel: t("Abbrechen") });
}

/** Opens the start page / new-project assistant. Unsaved changes are only
 *  asked about when something there replaces the current project. */
export function newProject() {
  wizardUi.open = true;
}

/** Before replacing the current project: asks about unsaved changes. */
export async function leaveDocument(): Promise<boolean> {
  if (!(await confirmDiscard())) return false;
  // Changes were given up on purpose: no recovery offer for them.
  if (editor.dirty && editor.objects.length) await clearAutosave();
  return true;
}

function projectData(): ProjectFile {
  const used = new Set(editor.objects.flatMap((o) => (o.type === "image" ? [o.imageId] : [])));
  const images: ProjectFile["images"] = {};
  for (const id of used) {
    const a = editor.images[id];
    if (a) images[id] = { name: a.name };
  }
  return {
    app: "schnittwerk",
    version: 1,
    mat: $state.snapshot(editor.mat),
    printCut: $state.snapshot(editor.printCut),
    guides: $state.snapshot(editor.guides),
    material: editor.material,
    kind: editor.projectKind,
    objects: $state.snapshot(editor.objects) as Obj[],
    images,
  };
}

/** Returns whether the project was written (false: cancelled or failed). */
export async function saveProject(saveAs = false): Promise<boolean> {
  let path = editor.filePath;
  if (!path || saveAs) {
    path = await save({ defaultPath: t("Projekt") + ".schnitt", filters: [{ name: t("Schnittwerk-Projekt"), extensions: [PROJECT_EXT] }] });
    if (!path) return false;
    if (!path.endsWith("." + PROJECT_EXT)) path += "." + PROJECT_EXT;
  }
  try {
    await api.projectSave(path, JSON.stringify(projectData()));
    editor.filePath = path;
    editor.dirty = false;
    editor.toast("Projekt gespeichert", "ok");
    clearAutosave();
    rememberRecent(path);
    return true;
  } catch (e) {
    editor.error(e);
    return false;
  }
}

/**
 * Window close: unsaved changes → ask to save. Returns whether the window
 * may close.
 */
export async function confirmClose(): Promise<boolean> {
  if (!editor.dirty || !editor.objects.length) return true;
  const name = editor.filePath?.split("/").pop() ?? "Unbenannt";
  const SAVE = "Speichern", DISCARD = "Nicht speichern", CANCEL = "Abbrechen";
  const r = await message(`Die Änderungen an „${name}“ sind nicht gespeichert. Vor dem Schließen speichern?`, {
    title: "Schnittwerk",
    kind: "warning",
    buttons: { yes: SAVE, no: DISCARD, cancel: CANCEL },
  });
  if (r === SAVE || r === "Yes") return saveProject();
  if (r === DISCARD || r === "No") {
    // Thrown away on purpose: no recovery offer next time.
    await clearAutosave();
    return true;
  }
  return false;
}

export async function openDialog() {
  if (!(await confirmDiscard())) return;
  const path = await open({ filters: [{ name: t("Schnittwerk-Projekt"), extensions: [PROJECT_EXT] }] });
  if (typeof path === "string") await openProject(path);
}

async function applyProject(file: ProjectFile, path: string | null) {
  if (file.app !== "schnittwerk") throw new Error("Keine Schnittwerk-Projektdatei.");
  // The backend has registered the pictures already (new ids).
  const remap: Record<string, string> = {};
  const images: typeof editor.images = {};
  for (const [oldId, img] of Object.entries(file.images)) {
    if (!img.id || !img.src) continue;
    remap[oldId] = img.id;
    images[img.id] = { src: img.src, name: img.name, pxW: img.width ?? 0, pxH: img.height ?? 0 };
  }
  editor.resetDocument();
  editor.images = images;
  editor.objects = file.objects.map((o) => (o.type === "image" ? { ...o, imageId: remap[o.imageId] ?? o.imageId } : o));
  editor.mat = MATS.find((m) => m.name === file.mat.name) ?? file.mat;
  const pc = file.printCut;
  editor.printCut = pc
    ? { enabled: pc.enabled, page: pc.page, insetX: pc.insetX ?? pc.inset ?? 15, insetY: pc.insetY ?? pc.inset ?? 10 }
    : { enabled: false, page: "A4", insetX: 15, insetY: 10 };
  editor.guides = { v: file.guides?.v ?? [], h: file.guides?.h ?? [] };
  editor.material = file.material ?? null;
  editor.projectKind = file.kind ?? null;
  editor.filePath = path;
  editor.fitMat();
}

/** Opens a project file (no confirmation – callers ask first). */
export async function openProject(path: string) {
  try {
    const file = (await api.projectOpen(path)) as ProjectFile;
    await applyProject(file, path);
    wizardUi.open = false;
    clearAutosave();
    rememberRecent(path);
  } catch (e) {
    forgetRecent(path);
    throw e;
  }
}

// ---------- autosave ----------

/** Seconds between recovery copies while there are unsaved changes. */
const AUTOSAVE_S = 20;
let savedRev = -1;
let autosaveTimer: ReturnType<typeof setInterval> | null = null;

export function startAutosave() {
  if (!inApp || autosaveTimer) return;
  autosaveTimer = setInterval(writeAutosave, AUTOSAVE_S * 1000);
}

async function writeAutosave() {
  if (!editor.dirty || !editor.objects.length || editor.rev === savedRev) return;
  const rev = editor.rev;
  try {
    const data: ProjectFile = { ...projectData(), autosave: { savedAt: Date.now(), filePath: editor.filePath } };
    await api.autosaveWrite(JSON.stringify(data));
    savedRev = rev;
  } catch {
    /* disk full or similar – try again next time */
  }
}

export function clearAutosave(): Promise<void> {
  savedRev = editor.rev;
  return inApp ? api.autosaveClear().catch(() => {}) : Promise.resolve();
}

export interface Recovery {
  savedAt: number;
  name: string;
  objects: number;
  filePath: string | null;
}

/** A recovery copy left over from a crash or an unsaved session
 *  (only its summary – the pictures are loaded when restoring). */
export async function findRecovery(): Promise<Recovery | null> {
  if (!inApp) return null;
  try {
    const info = await api.autosaveInfo();
    if (!info) return null;
    return { savedAt: info.savedAt, name: info.filePath?.split("/").pop() ?? "Unbenannt", objects: info.objects, filePath: info.filePath };
  } catch {
    return null;
  }
}

export async function restoreRecovery(r: Recovery) {
  const file = (await api.projectOpen(null)) as ProjectFile;
  await applyProject(file, r.filePath);
  // Still unsaved: keep the copy until the user saves.
  editor.dirty = true;
  savedRev = editor.rev;
  wizardUi.open = false;
  editor.toast("Nicht gespeicherte Arbeit wiederhergestellt", "ok");
}

// ---------- recent files ----------

const RECENT_KEY = "schnittwerk.recent";
const RECENT_MAX = 8;

export interface RecentFile {
  path: string;
  name: string;
  time: number;
  /** Small preview image (data URL). */
  thumb?: string;
}

export const recent = $state<{ list: RecentFile[] }>({ list: loadRecent() });

function loadRecent(): RecentFile[] {
  try {
    const l = JSON.parse(localStorage.getItem(RECENT_KEY) ?? "[]");
    return Array.isArray(l) ? l.filter((r) => typeof r?.path === "string") : [];
  } catch {
    return [];
  }
}

function storeRecent() {
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(recent.list));
  } catch {
    // Quota: drop the previews and try again.
    recent.list = recent.list.map(({ thumb: _, ...r }) => r);
    try {
      localStorage.setItem(RECENT_KEY, JSON.stringify(recent.list));
    } catch {}
  }
}

async function rememberRecent(path: string) {
  const name = path.split("/").pop()?.replace(/\.schnitt$/, "") ?? path;
  const old = recent.list.find((r) => r.path === path);
  recent.list = [{ path, name, time: Date.now(), thumb: old?.thumb }, ...recent.list.filter((r) => r.path !== path)].slice(0, RECENT_MAX);
  storeRecent();
  const thumb = await renderThumb().catch(() => undefined);
  const entry = recent.list.find((r) => r.path === path);
  if (entry && thumb) {
    entry.thumb = thumb;
    storeRecent();
  }
}

export function forgetRecent(path: string) {
  recent.list = recent.list.filter((r) => r.path !== path);
  storeRecent();
}

/** Renders the work area into a small JPEG for the recent files list. */
async function renderThumb(): Promise<string | undefined> {
  if (!editor.objects.length) return undefined;
  const draw = async (withImages: boolean) => {
    // Lines about 1.5 px wide in the small preview.
    const stroke = ((editor.mat.w / 160) * 1.5).toFixed(2);
    const svg = buildSvg(!withImages).replace("<svg ", '<svg style="background:#fff" ').replaceAll('stroke-width="0.25"', `stroke-width="${stroke}"`);
    const url = URL.createObjectURL(new Blob([svg], { type: "image/svg+xml" }));
    try {
      const img = new Image();
      await new Promise<void>((res, rej) => {
        img.onload = () => res();
        img.onerror = () => rej(new Error("Vorschau fehlgeschlagen"));
        img.src = url;
      });
      const W = 160, H = Math.round((W * editor.mat.h) / editor.mat.w);
      const c = document.createElement("canvas");
      c.width = W;
      c.height = Math.min(H, 400);
      const g = c.getContext("2d")!;
      g.fillStyle = "#fff";
      g.fillRect(0, 0, c.width, c.height);
      g.drawImage(img, 0, 0, W, H);
      return c.toDataURL("image/jpeg", 0.7);
    } finally {
      URL.revokeObjectURL(url);
    }
  };
  try {
    return await draw(true);
  } catch {
    // Embedded pictures may block reading the canvas: outlines only.
    return await draw(false);
  }
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
  let path = await save({ defaultPath: t("Schnittlinien") + ".svg", filters: [{ name: "SVG", extensions: ["svg"] }] });
  if (!path) return;
  if (!path.toLowerCase().endsWith(".svg")) path += ".svg";
  try {
    await api.writeText(path, buildSvg(true));
    editor.toast("SVG exportiert", "ok");
  } catch (e) {
    editor.error(e);
  }
}
