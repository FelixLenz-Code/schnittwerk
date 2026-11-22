import { center, rotate, uid } from "./geometry";
import type { ImageObj, PathObj, TraceLayer } from "./types";

/**
 * Turns a traced layer (in source-image pixels) into a path object that sits
 * exactly on top of the image, including its rotation and mirroring.
 */
export function layerToPath(img: ImageObj, srcW: number, srcH: number, l: TraceLayer, extra: Partial<PathObj> = {}): PathObj {
  const sx = img.w / srcW, sy = img.h / srcH;
  const [bx, by, bw, bh] = l.bbox;
  const w = bw * sx, h = bh * sy;
  let lx = (bx + bw / 2) * sx - img.w / 2;
  let ly = (by + bh / 2) * sy - img.h / 2;
  if (img.flipX) lx = -lx;
  if (img.flipY) ly = -ly;
  const off = rotate({ x: lx, y: ly }, img.rot);
  const c = center(img);
  return {
    id: uid(),
    type: "path",
    name: img.name + " · Kontur",
    d: l.d,
    bbox: l.bbox,
    color: "#e5484d",
    fill: false,
    x: c.x + off.x - w / 2,
    y: c.y + off.y - h / 2,
    w,
    h,
    rot: img.rot,
    flipX: img.flipX,
    flipY: img.flipY,
    visible: true,
    locked: false,
    ...extra,
  };
}
