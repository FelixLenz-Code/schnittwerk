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
  /** Straightened PixScan photo: where it was placed (mm), for calibration. */
  pixscan?: { x: number; y: number };
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
  /** Present on drawn shapes: the path is regenerated from it (e.g. corner radius). */
  shape?: ShapeSpec;
}

export type ShapeSpec =
  | { kind: "rect"; radius: number }
  | { kind: "ellipse" }
  | { kind: "polygon"; sides: number }
  | { kind: "star"; points: number; inner: number }
  | { kind: "heart" };

export type Obj = ImageObj | PathObj;

export interface ImageAsset {
  src: string;
  name: string;
  pxW: number;
  pxH: number;
}

export type ShapeTool = "rect" | "ellipse" | "polygon" | "star" | "heart";
export type Tool = "select" | "pan" | ShapeTool | "line" | "pen" | "text";

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
  /** Even out lighting (photos of drawings). */
  flatten: boolean;
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
  /** Only cut inside [x, y, w, h] (tiles of large designs). */
  clip?: [number, number, number, number] | null;
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
  /** Text along a path (SVG path data in mm); set from `curve` or a picked path. */
  path?: string | null;
  /** Shift along the path in % of its length. */
  pathOffset?: number;
  /** How `path` was made: arc over / under a circle, or a path of the design. */
  curve?: { kind: "arcTop" | "arcBottom"; radius: number } | { kind: "custom" } | null;
}

export interface BleDevice {
  id: string;
  name: string;
  address: string;
  rssi: number | null;
}
