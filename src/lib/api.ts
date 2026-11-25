import { invoke, isTauri } from "@tauri-apps/api/core";
import type { CutPlan, CutSettings, ImageInfo, ImportedVector, OffsetResult, PathInput, PathOptions, PlotterStatus, Printers, BleDevice, RegMarks, StickerParams, TextSpec, TraceParams, TraceResult, MotifMask } from "./types";

export const inApp = isTauri();

export interface UpdateAsset {
  name: string;
  url: string;
  size: number;
  digest: string | null;
}

export interface UpdateInfo {
  current: string;
  latest: string;
  newer: boolean;
  /** How this copy was installed. */
  kind: "deb" | "flatpak" | "manual";
  notes: string;
  url: string;
  asset: UpdateAsset | null;
}

function call<T>(cmd: string, args: Record<string, unknown>): Promise<T> {
  if (!inApp) return Promise.reject(new Error("Diese Funktion gibt es nur in der Desktop-App."));
  return invoke<T>(cmd, args);
}

export const api = {
  loadImage: (path: string) => call<ImageInfo>("load_image", { path }),
  loadImageData: (dataUrl: string, name: string) => call<ImageInfo>("load_image_data", { dataUrl, name }),
  traceImage: (id: string, params: TraceParams) => call<TraceResult>("trace_image", { id, params }),
  detectPage: (id: string) => call<[number, number][] | null>("detect_page", { id }),
  warpImage: (id: string, quad: [number, number][], aspect: number | null, name: string) =>
    call<ImageInfo>("warp_image", { id, quad, aspect, name }),
  detectPixscan: (id: string) => call<{ frame: [number, number][]; oriented: boolean } | null>("detect_pixscan", { id }),
  pixscanRectify: (id: string, frame: [number, number][], name: string) =>
    call<{ info: ImageInfo; margin: number; frameW: number; frameH: number }>("pixscan_rectify", { id, frame, name }),
  sampleColor: (id: string, x: number, y: number) =>
    call<[number, number, number]>("sample_color", { id, x: Math.round(x), y: Math.round(y) }),
  importSvg: (path: string) => call<ImportedVector>("import_svg", { path }),
  importDxf: (path: string) => call<ImportedVector>("import_dxf", { path }),
  importPdf: (path: string) =>
    call<{ kind: "vector"; vector: ImportedVector } | { kind: "image"; info: ImageInfo }>("import_pdf", { path, page: 1 }),
  readText: (path: string) => call<string>("read_text", { path }),
  writeText: (path: string, contents: string) => call<void>("write_text", { path, contents }),
  fileExists: (path: string) => call<boolean>("file_exists", { path }),
  updateCheck: () => call<UpdateInfo>("update_check", {}),
  updateInstall: (asset: UpdateAsset) => call<void>("update_install", { asset }),
  updateRestart: () => call<void>("update_restart", {}),
  openUrl: (url: string) => call<void>("open_url", { url }),
  autosaveWrite: (contents: string) => call<void>("autosave_write", { contents }),
  autosaveRead: () => call<string | null>("autosave_read", {}),
  autosaveClear: () => call<void>("autosave_clear", {}),
  simplifyPath: (d: string, tolerance: number, cornerDeg: number) =>
    call<{ d: string; nodesBefore: number; nodesAfter: number }>("simplify_path", { d, tolerance, cornerDeg }),

  plotterStatus: () => call<PlotterStatus>("plotter_status", {}),
  bleScan: () => call<BleDevice[]>("ble_scan", {}),
  bleConnect: (id: string) => call<string>("ble_connect", { id }),
  plotterDisconnect: () => call<void>("plotter_disconnect", {}),
  cutPreview: (groups: { paths: PathInput[]; options: PathOptions }[]) => call<CutPlan[]>("cut_preview", { groups }),
  cutStart: (groups: { paths: PathInput[]; options: PathOptions; settings: CutSettings }[], regmarks: RegMarks | null, feed: number | null = null) =>
    call<{ clamped: number }>("cut_start", { req: { groups, regmarks, feed } }),
  cutRegtest: (regmarks: RegMarks, settings: CutSettings) => call<{ clamped: number }>("cut_regtest", { regmarks, settings }),
  cutTest: (x: number, y: number, settings: CutSettings) => call<{ clamped: number }>("cut_test", { x, y, settings }),
  cutPause: (paused: boolean) => call<void>("cut_pause", { paused }),
  cutCancel: () => call<void>("cut_cancel", {}),

  stickerOutline: (id: string, params: StickerParams) => call<TraceResult>("sticker_outline", { id, params }),
  offsetPaths: (paths: PathInput[], offset: number, smoothness: number) =>
    call<OffsetResult>("offset_paths", { paths, offset, smoothness }),
  bleedImage: (id: string, mask: MotifMask, bleedPx: number) =>
    call<{ info: ImageInfo; pad: number }>("bleed_image", { id, mask, bleedPx: Math.round(bleedPx) }),
  clipboardRead: () =>
    call<{ kind: "image"; info: ImageInfo } | { kind: "svg"; vector: ImportedVector } | { kind: "none" }>("clipboard_read", {}),
  clipboardMark: () => call<void>("clipboard_mark", {}),
  fontFamilies: () => call<string[]>("font_families", {}),
  textToPath: (spec: TextSpec) => call<ImportedVector>("text_to_path", { spec }),
  nest: (req: {
    items: { paths: PathInput[] }[];
    area: [number, number, number, number];
    keepouts: [number, number, number, number][];
    gap: number;
    rotate: boolean;
    fill: boolean;
  }) =>
    call<{ placements: { item: number; rot: number; center: [number, number]; dx: number; dy: number }[]; unplaced: number[] }>("nest", { req }),
  booleanPaths: (paths: PathInput[], op: "union" | "difference" | "intersect" | "xor") =>
    call<OffsetResult>("boolean_paths", { paths, op }),
  listPrinters: () => call<Printers>("list_printers", {}),
  printPage: (png: string, pageWMm: number, pageHMm: number, media: string, printer: string | null, savePath: string | null) =>
    call<void>("print_page", { png, pageWMm, pageHMm, media, printer, savePath }),
};
