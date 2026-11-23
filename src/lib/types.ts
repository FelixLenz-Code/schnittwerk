export type Vec2 = { x: number; y: number };
/** [x, y, w, h] */
export type Bbox = [number, number, number, number];

interface BaseObj {
  id: string;
  name: string;
  /** Top-left of the unrotated box, in mm on the mat. */
  x: number;
  y: number;
  w: number;
  h: number;
  /** Degrees, clockwise, around the box center. */
  rot: number;
  flipX: boolean;
  flipY: boolean;
  visible: boolean;
  locked: boolean;
  /** Objects sharing a group id are selected and moved together. */
  group?: string;
}

export interface ImageObj extends BaseObj {
  type: "image";
  /** Key into editor.images (backend image id). */
  imageId: string;
  opacity: number;
}

export interface PathObj extends BaseObj {
  type: "path";
  /** Path data in local units; `bbox` is its extent in those units. */
  d: string;
  bbox: Bbox;
  color: string;
  fill: boolean;
  /** Printed fill colour (print & cut, e.g. a coloured sticker border). */
  printFill?: string | null;
  /** false = print only, not cut. */
  cut?: boolean;
  /** Present on text objects: the outlines are regenerated from it. */
  text?: TextSpec;
}

export type Obj = ImageObj | PathObj;

export interface ImageAsset {
  src: string;
  name: string;
  pxW: number;
  pxH: number;
}

export type Tool = "select" | "pan" | "rect" | "ellipse" | "text";

// ---- backend payloads ----

export interface ImageInfo {
  id: string;
  name: string;
  width: number;
  height: number;
  src: string;
}

export type TraceMode =
  | { kind: "threshold"; level: number | null; invert: boolean }
  | { kind: "colorKey"; color: [number, number, number]; tolerance: number }
  | { kind: "alpha" }
  | { kind: "multiColor"; colors: number; skipBackground: boolean }
  | { kind: "centerline"; level: number | null; invert: boolean };

export interface TraceParams {
  mode: TraceMode;
  brightness: number;
  contrast: number;
  blur: number;
  despeckle: number;
  smoothness: number;
  grow: number;
  outerOnly: boolean;
  curves: boolean;
  maxSize: number;
}

export interface TraceLayer {
  d: string;
  color: string;
  bbox: Bbox;
  contours: number;
}

export interface TraceResult {
  width: number;
  height: number;
  layers: TraceLayer[];
  threshold: number | null;
}

export interface ImportedVector {
  name: string;
  d: string;
  bbox: Bbox;
  pathCount: number;
}

// ---- cutting ----

export type ToolKind = "blade" | "pen";
export type MatKind = "portrait8x12" | "portrait8_5x12" | "none";

export interface CutSettings {
  tool: ToolKind;
  pressure: number;
  speed: number;
  depth: number | null;
  mat: MatKind;
  bladeOffset: number;
  liftCorners: boolean;
}

export interface PathOptions {
  overcut: number;
  passes: number;
  optimize: boolean;
}

export interface PathInput {
  d: string;
  matrix: [number, number, number, number, number, number];
}

export interface CutPlan {
  strokes: [number, number][][];
  cutLength: number;
  travelLength: number;
  bbox: Bbox;
}

export interface PlotterStatus {
  connected: boolean;
  model: string | null;
  connection: "USB" | "Bluetooth" | null;
  state: "ready" | "moving" | "unloaded" | "cutting" | "unknown" | "disconnected" | "error";
  message: string | null;
  widthMm: number | null;
}

// ---- stickers / print & cut ----

export type MotifMask = { kind: "alpha" } | { kind: "background"; tolerance: number };

export interface StickerParams {
  mask: MotifMask;
  offsetPx: number;
  smoothness: number;
  fillHoles: boolean;
  despeckle: number;
}

export interface OffsetResult {
  d: string;
  bbox: Bbox;
  contours: number;
}

export interface RegMarks {
  originX: number;
  originY: number;
  width: number;
  length: number;
}

export interface Printers {
  names: string[];
  default: string | null;
}

export interface TextSpec {
  content: string;
  family: string;
  /** Font size in mm. */
  size: number;
  bold: boolean;
  italic: boolean;
  /** Extra letter spacing in mm. */
  spacing: number;
  lineHeight: number;
  align: "start" | "middle" | "end";
  /** Merge overlapping letters into one outline (script fonts). */
  weld: boolean;
}

export interface BleDevice {
  id: string;
  name: string;
  address: string;
  rssi: number | null;
}
