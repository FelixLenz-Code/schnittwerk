import type { ToolKind } from "./types";

export interface Material {
  id: string;
  name: string;
  tool: ToolKind;
  /** AutoBlade depth 1..10 */
  depth: number;
  /** Force 1..33 */
  pressure: number;
  /** Speed 1..30 */
  speed: number;
  passes: number;
  /** Overcut in mm */
  overcut: number;
  /** Mirror the design (heat transfer vinyl). */
  mirror: boolean;
  custom?: boolean;
}

/** Starting values – always confirm with a test cut. */
export const PRESETS: Material[] = [
  { id: "vinyl", name: "Vinyl / Klebefolie", tool: "blade", depth: 1, pressure: 10, speed: 5, passes: 1, overcut: 0.3, mirror: false },
  { id: "sticker-matt", name: "Sticker-Papier matt", tool: "blade", depth: 2, pressure: 14, speed: 5, passes: 1, overcut: 0.3, mirror: false },
  { id: "sticker-glossy", name: "Sticker-Papier glänzend", tool: "blade", depth: 2, pressure: 16, speed: 5, passes: 1, overcut: 0.3, mirror: false },
  { id: "sticker-vinyl", name: "Sticker-Folie (bedruckbar)", tool: "blade", depth: 2, pressure: 12, speed: 5, passes: 1, overcut: 0.3, mirror: false },
  { id: "paper", name: "Kopierpapier 80 g", tool: "blade", depth: 2, pressure: 14, speed: 8, passes: 1, overcut: 0.3, mirror: false },
  { id: "photo", name: "Fotopapier", tool: "blade", depth: 3, pressure: 20, speed: 5, passes: 1, overcut: 0.3, mirror: false },
  { id: "cardstock", name: "Karton 200–250 g", tool: "blade", depth: 5, pressure: 27, speed: 3, passes: 2, overcut: 0.5, mirror: false },
  { id: "htv", name: "Flexfolie (Bügelfolie)", tool: "blade", depth: 2, pressure: 9, speed: 8, passes: 1, overcut: 0.3, mirror: true },
  { id: "transparency", name: "Transparentfolie", tool: "blade", depth: 2, pressure: 12, speed: 5, passes: 1, overcut: 0.3, mirror: false },
  { id: "magnet", name: "Magnetfolie", tool: "blade", depth: 9, pressure: 30, speed: 3, passes: 2, overcut: 0.5, mirror: false },
  { id: "pen", name: "Stift (Zeichnen)", tool: "pen", depth: 0, pressure: 18, speed: 10, passes: 1, overcut: 0, mirror: false },
];

const KEY = "schnittwerk.materials";
const LAST = "schnittwerk.lastMaterial";

export function loadCustom(): Material[] {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "[]") as Material[];
  } catch {
    return [];
  }
}

export function saveCustom(list: Material[]) {
  try {
    localStorage.setItem(KEY, JSON.stringify(list));
  } catch {}
}

export function lastMaterialId(): string {
  try {
    return localStorage.getItem(LAST) ?? "vinyl";
  } catch {
    return "vinyl";
  }
}

export function rememberMaterial(id: string) {
  try {
    localStorage.setItem(LAST, id);
  } catch {}
}
