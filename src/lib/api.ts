import { invoke, isTauri } from "@tauri-apps/api/core";
import type { CutPlan, CutSettings, ImageInfo, ImportedVector, OffsetResult, PathInput, PathOptions, PlotterStatus, Printers, RegMarks, StickerParams, TextSpec, TraceParams, TraceResult, MotifMask } from "./types";

export const inApp = isTauri();

function call<T>(cmd: string, args: Record<string, unknown>): Promise<T> {
  if (!inApp) return Promise.reject(new Error("Diese Funktion gibt es nur in der Desktop-App."));
  return invoke<T>(cmd, args);
}

export const api = {
  loadImage: (path: string) => call<ImageInfo>("load_image", { path }),
  loadImageData: (dataUrl: string, name: string) => call<ImageInfo>("load_image_data", { dataUrl, name }),
  traceImage: (id: string, params: TraceParams) => call<TraceResult>("trace_image", { id, params }),
  sampleColor: (id: string, x: number, y: number) =>
    call<[number, number, number]>("sample_color", { id, x: Math.round(x), y: Math.round(y) }),
  importSvg: (path: string) => call<ImportedVector>("import_svg", { path }),
  importDxf: (path: string) => call<ImportedVector>("import_dxf", { path }),
  importPdf: (path: string) =>
    call<{ kind: "vector"; vector: ImportedVector } | { kind: "image"; info: ImageInfo }>("import_pdf", { path, page: 1 }),
  readText: (path: string) => call<string>("read_text", { path }),
  writeText: (path: string, contents: string) => call<void>("write_text", { path, contents }),

  plotterStatus: () => call<PlotterStatus>("plotter_status", {}),
  cutPreview: (groups: { paths: PathInput[]; options: PathOptions }[]) => call<CutPlan[]>("cut_preview", { groups }),
  cutStart: (groups: { paths: PathInput[]; options: PathOptions; settings: CutSettings }[], regmarks: RegMarks | null) =>
    call<{ clamped: number }>("cut_start", { req: { groups, regmarks } }),
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
  listPrinters: () => call<Printers>("list_printers", {}),
  printPage: (png: string, pageWMm: number, pageHMm: number, media: string, printer: string | null, savePath: string | null) =>
    call<void>("print_page", { png, pageWMm, pageHMm, media, printer, savePath }),
};
