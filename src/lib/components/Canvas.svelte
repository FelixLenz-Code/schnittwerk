<script lang="ts">
  import { editor, regBracketPath } from "../editor.svelte";
  import { aabb, boxTransform, center, corners, ellipsePath, fromLocal, rectPath, rotate, toLocal, uid } from "../geometry";
  import type { Obj, PathObj, Vec2 } from "../types";
  import { createText } from "../text.svelte";
  import { brush } from "../brush.svelte";
  import { perspective } from "../perspective.svelte";
  import { key, nodes, type NodeKey } from "../nodes.svelte";
  import { nearestSegment, type PNode } from "../pathdata";

  type Drag =
    | { kind: "pan"; sx: number; sy: number; panX: number; panY: number }
    | { kind: "move"; start: Vec2; orig: Map<string, Vec2>; moved: boolean; axisLock: boolean }
    | { kind: "scale"; id: string; hx: number; hy: number; w0: number; h0: number; anchor: Vec2; rot: number; moved: boolean; free: boolean }
    | { kind: "rotate"; id: string; c: Vec2; a0: number; rot0: number; moved: boolean }
    | { kind: "marquee"; start: Vec2; cur: Vec2; additive: boolean }
    | { kind: "draw"; shape: "rect" | "ellipse"; start: Vec2; cur: Vec2 }
    | { kind: "brush"; id: string }
    | { kind: "persp"; i: number }
    | { kind: "nodes"; start: Vec2; orig: Map<NodeKey, PNode>; moved: boolean }
    | { kind: "nodeh"; k: NodeKey; side: "cin" | "cout"; moved: boolean }
    | { kind: "nodeMarquee"; start: Vec2; cur: Vec2; additive: boolean };

  let svg: SVGSVGElement;
  let drag = $state<Drag | null>(null);
  let spaceDown = $state(false);

  const z = $derived(editor.view.zoom);
  const px = (n: number) => n / z; // screen px → mm

  function toWorld(e: { clientX: number; clientY: number }): Vec2 {
    const r = svg.getBoundingClientRect();
    return { x: (e.clientX - r.left - editor.view.panX) / z, y: (e.clientY - r.top - editor.view.panY) / z };
  }

  function pathTransform(o: PathObj) {
    const [bx, by, bw, bh] = o.bbox;
    return `${boxTransform(o)} scale(${o.w / (bw || 1)} ${o.h / (bh || 1)}) translate(${-bx} ${-by})`;
  }

  // ---------- pointer handling ----------

  function onPointerDown(e: PointerEvent) {
    svg.setPointerCapture(e.pointerId);
    const p = toWorld(e);
    const target = e.target as Element;

    if (e.button === 1 || editor.tool === "pan" || spaceDown) {
      drag = { kind: "pan", sx: e.clientX, sy: e.clientY, panX: editor.view.panX, panY: editor.view.panY };
      return;
    }
    if (e.button !== 0) return;

    // Node editing: nodes, handles, or a marquee to select nodes.
    if (nodes.objId) {
      const hk = target.closest("[data-nh]")?.getAttribute("data-nh");
      if (hk) {
        const [si, ni, side] = hk.split(":");
        drag = { kind: "nodeh", k: key(+si, +ni), side: side as "cin" | "cout", moved: false };
        return;
      }
      const nk = target.closest("[data-node]")?.getAttribute("data-node");
      if (nk) {
        if (e.shiftKey) nodes.sel = nodes.sel.includes(nk) ? nodes.sel.filter((k) => k !== nk) : [...nodes.sel, nk];
        else if (!nodes.sel.includes(nk)) nodes.sel = [nk];
        const orig = new Map(nodes.sel.map((k) => [k, $state.snapshot(nodes.node(k)!) as PNode]));
        drag = { kind: "nodes", start: nodes.fromWorld(p), orig, moved: false };
        return;
      }
      drag = { kind: "nodeMarquee", start: p, cur: p, additive: e.shiftKey };
      return;
    }

    // Perspective correction: only the four corner handles react.
    if (perspective.objId) {
      const h = target.closest("[data-persp]")?.getAttribute("data-persp");
      if (h !== null && h !== undefined) drag = { kind: "persp", i: Number(h) };
      return;
    }

    // Eraser / paint brush on the selected image (trace panel).
    if (brush.mode && !brush.busy && brushTarget) {
      const img = brushTarget;
      const a = editor.images[img.imageId];
      if (a) {
        const l = toLocal(img, p);
        brush.stroke = [{ x: (l.x / img.w) * a.pxW, y: (l.y / img.h) * a.pxH }];
        drag = { kind: "brush", id: img.id };
      }
      return;
    }

    // Color picking for the trace panel.
    if (editor.picking && editor.tracePreview) {
      const o = editor.byId(editor.tracePreview.objId);
      if (o?.type === "image") {
        const a = editor.images[o.imageId];
        const l = toLocal(o, p);
        if (a && l.x >= 0 && l.y >= 0 && l.x <= o.w && l.y <= o.h) {
          editor.picking((l.x / o.w) * a.pxW, (l.y / o.h) * a.pxH);
        }
      }
      editor.picking = null;
      return;
    }

    if (editor.tool === "text") {
      createText(p);
      editor.tool = "select";
      return;
    }

    if (editor.tool === "rect" || editor.tool === "ellipse") {
      drag = { kind: "draw", shape: editor.tool, start: p, cur: p };
      return;
    }

    const handle = target.closest("[data-handle]")?.getAttribute("data-handle");
    if (handle && editor.selected.length === 1) {
      const o = editor.selected[0];
      if (o.locked) return;
      if (handle === "rot") {
        const c = center(o);
        drag = { kind: "rotate", id: o.id, c, a0: Math.atan2(p.y - c.y, p.x - c.x), rot0: o.rot, moved: false };
      } else {
        const [hx, hy] = handle.split(",").map(Number);
        const c = center(o);
        const off = rotate({ x: (-hx * o.w) / 2, y: (-hy * o.h) / 2 }, o.rot);
        drag = {
          kind: "scale", id: o.id, hx, hy, w0: o.w, h0: o.h, rot: o.rot, moved: false, free: e.shiftKey,
          anchor: { x: c.x + off.x, y: c.y + off.y },
        };
      }
      return;
    }

    const objId = target.closest("[data-obj]")?.getAttribute("data-obj");
    const obj = objId ? editor.byId(objId) : undefined;
    if (obj && !obj.locked) {
      const unit = editor.expandGroups([obj.id]);
      if (e.shiftKey) {
        editor.selection = editor.selection.includes(obj.id)
          ? editor.selection.filter((id) => !unit.includes(id))
          : [...new Set([...editor.selection, ...unit])];
      } else if (!editor.selection.includes(obj.id)) {
        editor.selection = unit;
      }
      const orig = new Map(editor.selected.filter((o) => !o.locked).map((o) => [o.id, { x: o.x, y: o.y }]));
      drag = { kind: "move", start: p, orig, moved: false, axisLock: false };
      return;
    }

    drag = { kind: "marquee", start: p, cur: p, additive: e.shiftKey };
    if (!e.shiftKey) editor.selection = [];
  }

  function onPointerMove(e: PointerEvent) {
    const p = toWorld(e);
    editor.cursor = p;
    if (!drag) return;

    switch (drag.kind) {
      case "pan":
        editor.view.panX = drag.panX + e.clientX - drag.sx;
        editor.view.panY = drag.panY + e.clientY - drag.sy;
        break;

      case "move": {
        let dx = p.x - drag.start.x, dy = p.y - drag.start.y;
        if (!drag.moved) {
          if (Math.hypot(dx, dy) * z < 3) return;
          editor.checkpoint();
          drag.moved = true;
        }
        if (e.shiftKey) Math.abs(dx) > Math.abs(dy) ? (dy = 0) : (dx = 0);
        for (const o of editor.selected) {
          const s = drag.orig.get(o.id);
          if (s) {
            o.x = s.x + dx;
            o.y = s.y + dy;
          }
        }
        break;
      }

      case "scale": {
        const o = editor.byId(drag.id);
        if (!o) return;
        if (!drag.moved) {
          editor.checkpoint();
          drag.moved = true;
        }
        const v = rotate({ x: p.x - drag.anchor.x, y: p.y - drag.anchor.y }, -drag.rot);
        let w = drag.w0, h = drag.h0;
        const corner = drag.hx !== 0 && drag.hy !== 0;
        if (corner && !e.shiftKey) {
          // Proportional: project the drag onto the box diagonal.
          const dir = { x: drag.hx * drag.w0, y: drag.hy * drag.h0 };
          const s = Math.max(0.01, (v.x * dir.x + v.y * dir.y) / (dir.x * dir.x + dir.y * dir.y));
          w = drag.w0 * s;
          h = drag.h0 * s;
        } else {
          if (drag.hx !== 0) w = Math.max(0.5, v.x * drag.hx);
          if (drag.hy !== 0) h = Math.max(0.5, v.y * drag.hy);
        }
        const off = rotate({ x: (drag.hx * w) / 2, y: (drag.hy * h) / 2 }, drag.rot);
        const c = { x: drag.anchor.x + off.x, y: drag.anchor.y + off.y };
        o.w = w;
        o.h = h;
        o.x = c.x - w / 2;
        o.y = c.y - h / 2;
        break;
      }

      case "rotate": {
        const o = editor.byId(drag.id);
        if (!o) return;
        if (!drag.moved) {
          editor.checkpoint();
          drag.moved = true;
        }
        let r = drag.rot0 + ((Math.atan2(p.y - drag.c.y, p.x - drag.c.x) - drag.a0) * 180) / Math.PI;
        if (e.shiftKey) r = Math.round(r / 15) * 15;
        o.rot = ((r % 360) + 360) % 360;
        break;
      }

      case "marquee":
      case "draw":
        drag.cur = p;
        break;

      case "nodes": {
        const q = nodes.fromWorld(p);
        let dx = q.x - drag.start.x, dy = q.y - drag.start.y;
        if (!drag.moved) {
          if (Math.hypot(p.x - nodes.toWorld(drag.start).x, p.y - nodes.toWorld(drag.start).y) * z < 3) return;
          editor.checkpoint();
          drag.moved = true;
        }
        if (e.shiftKey) Math.abs(dx) > Math.abs(dy) ? (dy = 0) : (dx = 0);
        nodes.moveSelected(drag.orig, dx, dy);
        break;
      }

      case "nodeh": {
        if (!drag.moved) {
          editor.checkpoint();
          drag.moved = true;
          // Alt: move this handle on its own (makes a corner).
          const n = nodes.node(drag.k);
          if (n && e.altKey) n.smooth = false;
        }
        nodes.moveHandle(drag.k, drag.side, nodes.fromWorld(p));
        break;
      }

      case "nodeMarquee":
        drag.cur = p;
        break;

      case "persp": {
        const img = perspImg;
        const a = img ? editor.images[img.imageId] : undefined;
        if (!img || !a) return;
        const l = toLocal(img, p);
        perspective.pts[drag.i] = {
          x: Math.min(a.pxW, Math.max(0, (l.x / img.w) * a.pxW)),
          y: Math.min(a.pxH, Math.max(0, (l.y / img.h) * a.pxH)),
        };
        break;
      }

      case "brush": {
        const img = editor.byId(drag.id);
        if (img?.type !== "image") return;
        const a = editor.images[img.imageId];
        if (!a) return;
        const l = toLocal(img, p);
        const q = { x: (l.x / img.w) * a.pxW, y: (l.y / img.h) * a.pxH };
        const last = brush.stroke[brush.stroke.length - 1];
        if (!last || Math.hypot(q.x - last.x, q.y - last.y) > 0.5) brush.stroke.push(q);
        break;
      }
    }
  }

  function onPointerUp(e: PointerEvent) {
    svg.releasePointerCapture(e.pointerId);
    if (drag?.kind === "nodeMarquee") {
      const x0 = Math.min(drag.start.x, drag.cur.x), x1 = Math.max(drag.start.x, drag.cur.x);
      const y0 = Math.min(drag.start.y, drag.cur.y), y1 = Math.max(drag.start.y, drag.cur.y);
      if ((x1 - x0) * z > 3 || (y1 - y0) * z > 3) {
        const hit: NodeKey[] = [];
        nodes.subs.forEach((s, si) =>
          s.nodes.forEach((n, ni) => {
            const w = nodes.toWorld(n);
            if (w.x >= x0 && w.x <= x1 && w.y >= y0 && w.y <= y1) hit.push(key(si, ni));
          }),
        );
        nodes.sel = drag.additive ? [...new Set([...nodes.sel, ...hit])] : hit;
      } else if (!drag.additive) {
        nodes.sel = [];
      }
    } else if (drag?.kind === "brush") {
      const img = editor.byId(drag.id);
      if (img?.type === "image") brush.commit(img);
    } else if (drag?.kind === "marquee") {
      const x0 = Math.min(drag.start.x, drag.cur.x), x1 = Math.max(drag.start.x, drag.cur.x);
      const y0 = Math.min(drag.start.y, drag.cur.y), y1 = Math.max(drag.start.y, drag.cur.y);
      if ((x1 - x0) * z > 3 || (y1 - y0) * z > 3) {
        const hit = editor.objects
          .filter((o) => o.visible && !o.locked)
          .filter((o) => {
            const b = aabb([o])!;
            return b.x0 >= x0 && b.x1 <= x1 && b.y0 >= y0 && b.y1 <= y1;
          })
          .map((o) => o.id);
        const unit = editor.expandGroups(hit);
        editor.selection = drag.additive ? [...new Set([...editor.selection, ...unit])] : unit;
      }
    } else if (drag?.kind === "draw") {
      const x = Math.min(drag.start.x, drag.cur.x), y = Math.min(drag.start.y, drag.cur.y);
      let w = Math.abs(drag.cur.x - drag.start.x), h = Math.abs(drag.cur.y - drag.start.y);
      if (e.shiftKey) w = h = Math.max(w, h);
      if (w > 1 && h > 1) {
        const ell = drag.shape === "ellipse";
        const obj: PathObj = {
          id: uid(), type: "path", name: ell ? "Ellipse" : "Rechteck",
          d: ell ? ellipsePath(w, h) : rectPath(w, h), bbox: [0, 0, w, h],
          color: "#e5484d", fill: false, x, y, w, h, rot: 0, flipX: false, flipY: false, visible: true, locked: false,
        };
        editor.add([obj]);
        editor.tool = "select";
      }
    }
    drag = null;
  }

  /** Double-click: path → node editing; in node editing, on the outline →
   *  insert a node, elsewhere → done. */
  function onDblClick(e: MouseEvent) {
    const p = toWorld(e);
    // With pointer capture the event targets the svg itself: hit-test instead.
    const target = document.elementFromPoint(e.clientX, e.clientY) ?? (e.target as Element);
    if (nodes.objId) {
      if (target.closest("[data-node],[data-nh]")) return;
      const o = nodes.obj;
      if (!o) return;
      const hit = nearestSegment(nodes.subs, nodes.fromWorld(p));
      const scale = (o.w / (o.bbox[2] || 1) + o.h / (o.bbox[3] || 1)) / 2;
      if (hit && hit.dist * scale <= px(8)) nodes.insertAt(hit.si, hit.i, hit.t);
      else nodes.stop();
      return;
    }
    if (perspective.objId || brush.mode || editor.picking || editor.tool !== "select") return;
    const id = target.closest("[data-obj]")?.getAttribute("data-obj");
    const o = id ? editor.byId(id) : undefined;
    if (o?.type === "path" && !o.locked) nodes.start(o);
  }

  // Undo/redo or outside changes re-read the edited path; leaving the
  // selection or switching tools ends node editing.
  $effect(() => {
    if (!nodes.objId) return;
    void nodeObj?.d;
    nodes.sync();
  });
  $effect(() => {
    if (nodes.objId && (!editor.selection.includes(nodes.objId) || editor.tool !== "select")) nodes.stop();
  });

  function onWheel(e: WheelEvent) {
    e.preventDefault();
    const r = svg.getBoundingClientRect();
    if (e.shiftKey && !e.ctrlKey) {
      editor.view.panX -= e.deltaY || e.deltaX;
      return;
    }
    const factor = Math.exp(-e.deltaY * (e.ctrlKey ? 0.01 : 0.0018));
    editor.zoomAt(factor, e.clientX - r.left, e.clientY - r.top);
  }

  function onKey(e: KeyboardEvent, down: boolean) {
    const t = e.target;
    const typing = t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement || t instanceof HTMLSelectElement;
    if (e.code === "Space" && !typing) {
      spaceDown = down;
      if (down) e.preventDefault();
    }
  }

  // ---------- rulers ----------

  function rulerStep(): number {
    for (const s of [1, 2, 5, 10, 20, 50, 100, 200]) if (s * z >= 46) return s;
    return 500;
  }
  const step = $derived(rulerStep());
  function ticks(length: number, pan: number): number[] {
    const from = Math.floor(-pan / z / step) * step;
    const to = (length - pan) / z;
    const out: number[] = [];
    for (let v = from; v <= to; v += step) out.push(v);
    return out;
  }
  const RULER = 22;

  const brushTarget = $derived.by(() => {
    if (!brush.mode || editor.rightTab !== "trace") return null;
    const imgs = editor.selected.filter((o) => o.type === "image");
    return imgs.length === 1 && imgs[0].type === "image" ? imgs[0] : null;
  });

  const cursorStyle = $derived(
    drag?.kind === "pan" ? "grabbing"
      : editor.tool === "pan" || spaceDown ? "grab"
      : editor.picking ? "crosshair"
      : perspective.objId ? "default"
      : brush.mode && brushTarget ? "crosshair"
      : editor.tool === "rect" || editor.tool === "ellipse" ? "crosshair"
      : editor.tool === "text" ? "text"
      : "default",
  );

  const perspImg = $derived.by(() => {
    const o = perspective.objId ? editor.byId(perspective.objId) : undefined;
    return o?.type === "image" ? o : null;
  });
  /** Perspective corners in world coordinates. */
  const perspWorld = $derived.by(() => {
    const img = perspImg;
    const a = img ? editor.images[img.imageId] : undefined;
    if (!img || !a) return [];
    return perspective.pts.map((q) => fromLocal(img, { x: (q.x / a.pxW) * img.w, y: (q.y / a.pxH) * img.h }));
  });

  const nodeObj = $derived(nodes.obj);

  const single = $derived(perspective.objId || nodes.objId ? null : editor.selected.length === 1 ? editor.selected[0] : null);
  const multiBox = $derived(editor.selected.length > 1 ? aabb(editor.selected) : null);
  const previewObj = $derived(editor.tracePreview ? editor.byId(editor.tracePreview.objId) : undefined);
  const HANDLES: [number, number][] = [[-1, -1], [0, -1], [1, -1], [1, 0], [1, 1], [0, 1], [-1, 1], [-1, 0]];
</script>

<svelte:window onkeydown={(e) => onKey(e, true)} onkeyup={(e) => onKey(e, false)} />

<div class="wrap" bind:clientWidth={editor.viewport.w} bind:clientHeight={editor.viewport.h}>
  <svg
    bind:this={svg}
    role="application"
    aria-label="Arbeitsfläche"
    style:cursor={cursorStyle}
    onpointerdown={onPointerDown}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}
    onpointerleave={() => (editor.cursor = null)}
    onwheel={onWheel}
    ondblclick={onDblClick}
    oncontextmenu={(e) => e.preventDefault()}
  >
    <defs>
      <pattern id="grid-minor" width="10" height="10" patternUnits="userSpaceOnUse">
        <path d="M10 0H0V10" fill="none" stroke="var(--grid)" stroke-width={px(1)} />
      </pattern>
      <pattern id="grid-major" width="50" height="50" patternUnits="userSpaceOnUse">
        <rect width="50" height="50" fill="url(#grid-minor)" />
        <path d="M50 0H0V50" fill="none" stroke="var(--grid-major)" stroke-width={px(1)} />
      </pattern>
      <pattern id="keepout" width="4" height="4" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
        <rect width="4" height="4" fill="var(--danger)" opacity="0.06" />
        <line x1="0" y1="0" x2="0" y2="4" stroke="var(--danger)" stroke-width="0.6" opacity="0.35" />
      </pattern>
      <filter id="mat-shadow" x="-10%" y="-10%" width="120%" height="120%">
        <feDropShadow dx="0" dy={px(2)} stdDeviation={px(8)} flood-opacity="0.12" />
      </filter>
    </defs>

    <g transform="translate({editor.view.panX} {editor.view.panY}) scale({z})">
      <!-- Mat -->
      <rect x="0" y="0" width={editor.mat.w} height={editor.mat.h} fill={editor.printCut.enabled ? "#ffffff" : "var(--mat)"} filter="url(#mat-shadow)" />
      {#if editor.showGrid}
        <rect x="0" y="0" width={editor.mat.w} height={editor.mat.h} fill="url(#grid-major)" />
      {/if}
      <rect x="0" y="0" width={editor.mat.w} height={editor.mat.h} fill="none" stroke="var(--mat-border)" stroke-width={px(1)} />

      <!-- Objects -->
      {#each editor.objects as o (o.id)}
        {#if o.visible}
          {#if o.type === "image"}
            {@const a = editor.images[o.imageId]}
            <g data-obj={o.id} transform={boxTransform(o)} opacity={o.opacity}>
              {#if a}
                <image href={a.src} width={o.w} height={o.h} preserveAspectRatio="none" />
              {/if}
              <rect width={o.w} height={o.h} fill="transparent" />
            </g>
          {:else}
            <g data-obj={o.id}>
              <path
                d={o.d}
                transform={pathTransform(o)}
                fill={o.printFill ?? (o.fill ? o.color : "transparent")}
                fill-opacity={o.printFill ? 1 : o.fill ? 0.25 : 1}
                stroke-dasharray={o.cut === false ? "4 3" : undefined}
                fill-rule="evenodd"
                stroke={o.color}
                stroke-width="1.4"
                vector-effect="non-scaling-stroke"
              />
              <rect width={o.w} height={o.h} transform={boxTransform(o)} fill="transparent" pointer-events={o.fill || o.printFill ? "none" : "all"} opacity="0" />
            </g>
          {/if}
        {/if}
      {/each}

      <!-- Print & cut: registration marks and keep-out zones -->
      {#if editor.regmarks}
        {@const r = editor.regmarks}
        <g pointer-events="none">
          <path
            fill-rule="evenodd"
            fill="url(#keepout)"
            d="M0 0H{editor.mat.w}V{editor.mat.h}H0Z M{r.originX} {r.originY}H{r.originX + r.width}V{r.originY + r.length}H{r.originX}Z"
          />
          {#each editor.regZones() as [x, y, w, h]}
            <rect {x} {y} width={w} height={h} fill="url(#keepout)" />
          {/each}
          <rect x={r.originX} y={r.originY} width="5" height="5" fill="#000" />
          <path d={regBracketPath(r)} fill="none" stroke="#000" stroke-width="0.5" stroke-linejoin="miter" />
        </g>
      {/if}

      <!-- Live trace preview -->
      {#if editor.tracePreview && previewObj?.type === "image" && !perspective.objId}
        {@const r = editor.tracePreview.result}
        <g transform="{boxTransform(previewObj)} scale({previewObj.w / r.width} {previewObj.h / r.height})" pointer-events="none">
          {#each r.layers as l}
            <path d={l.d} fill={l.color} fill-opacity="0.35" fill-rule="evenodd" stroke="var(--accent)" stroke-width="1.5" vector-effect="non-scaling-stroke" />
          {/each}
        </g>
      {/if}

      <!-- Brush stroke in progress and brush cursor -->
      {#if brushTarget && brush.stroke.length}
        {@const a = editor.images[brushTarget.imageId]}
        {#if a}
          <g transform="{boxTransform(brushTarget)} scale({brushTarget.w / a.pxW} {brushTarget.h / a.pxH})" pointer-events="none">
            <polyline
              points={brush.stroke.map((q) => `${q.x},${q.y}`).join(" ")}
              fill="none"
              stroke={brush.mode === "erase" ? "#ffffff" : brush.color}
              stroke-opacity={brush.mode === "erase" ? 0.85 : 1}
              stroke-width={brush.sizePx(brushTarget)}
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </g>
        {/if}
      {/if}
      {#if brushTarget && editor.cursor}
        <circle cx={editor.cursor.x} cy={editor.cursor.y} r={brush.size / 2} fill="none" stroke="var(--accent)" stroke-width={px(1.2)} pointer-events="none" />
      {/if}

      <!-- Node editing -->
      {#if nodeObj}
        <path d={nodeObj.d} transform={pathTransform(nodeObj)} fill="none" stroke="var(--accent)" stroke-width="1.6" vector-effect="non-scaling-stroke" pointer-events="none" />
        <g>
          {#each nodes.subs as s, si}
            {#each s.nodes as n, ni}
              {@const k = key(si, ni)}
              {@const on = nodes.sel.includes(k)}
              {@const w = nodes.toWorld(n)}
              {#if on}
                {#each ["cin", "cout"] as const as side}
                  {@const c = n[side]}
                  {#if c}
                    {@const cw = nodes.toWorld(c)}
                    <line x1={w.x} y1={w.y} x2={cw.x} y2={cw.y} stroke="var(--accent)" stroke-width={px(1)} pointer-events="none" />
                    <circle data-nh="{si}:{ni}:{side}" cx={cw.x} cy={cw.y} r={px(4)} fill="var(--panel)" stroke="var(--accent)" stroke-width={px(1.5)} style:cursor="move" />
                  {/if}
                {/each}
              {/if}
              {#if n.smooth}
                <circle data-node={k} cx={w.x} cy={w.y} r={px(4)} fill={on ? "var(--accent)" : "var(--panel)"} stroke="var(--accent)" stroke-width={px(1.5)} style:cursor="move" />
              {:else}
                <rect data-node={k} x={w.x - px(3.5)} y={w.y - px(3.5)} width={px(7)} height={px(7)} fill={on ? "var(--accent)" : "var(--panel)"} stroke="var(--accent)" stroke-width={px(1.5)} style:cursor="move" />
              {/if}
            {/each}
          {/each}
        </g>
      {/if}
      {#if drag?.kind === "nodeMarquee"}
        {@const x = Math.min(drag.start.x, drag.cur.x)}
        {@const y = Math.min(drag.start.y, drag.cur.y)}
        <rect {x} {y} width={Math.abs(drag.cur.x - drag.start.x)} height={Math.abs(drag.cur.y - drag.start.y)} fill="var(--accent-soft)" stroke="var(--accent)" stroke-width={px(1)} pointer-events="none" />
      {/if}

      <!-- Perspective correction -->
      {#if perspWorld.length === 4}
        {@const pts = perspWorld.map((q) => `${q.x},${q.y}`).join(" ")}
        <g>
          <polygon points={pts} fill="var(--accent)" fill-opacity="0.08" stroke="var(--accent)" stroke-width={px(1.5)} pointer-events="none" />
          {#each [0, 1] as k}
            <!-- Guide lines through the middle help to line up the edges -->
            {@const [a, b, c, d] = k === 0 ? [0, 1, 3, 2] : [0, 3, 1, 2]}
            <line
              x1={(perspWorld[a].x + perspWorld[c].x) / 2}
              y1={(perspWorld[a].y + perspWorld[c].y) / 2}
              x2={(perspWorld[b].x + perspWorld[d].x) / 2}
              y2={(perspWorld[b].y + perspWorld[d].y) / 2}
              stroke="var(--accent)"
              stroke-width={px(1)}
              stroke-dasharray="{px(4)} {px(4)}"
              pointer-events="none"
            />
          {/each}
          {#each perspWorld as q, i}
            <circle data-persp={i} cx={q.x} cy={q.y} r={px(8)} fill={perspective.custom && i === 0 ? "var(--accent)" : "var(--panel)"} fill-opacity="0.6" stroke="var(--accent)" stroke-width={px(2)} style:cursor="move" />
            <circle cx={q.x} cy={q.y} r={px(1.5)} fill="var(--accent)" pointer-events="none" />
          {/each}
        </g>
      {/if}

      <!-- Selection -->
      {#each editor.selected as o (o.id)}
        {#if editor.selected.length > 1}
          <polygon points={corners(o).map((c) => `${c.x},${c.y}`).join(" ")} fill="none" stroke="var(--accent)" stroke-width={px(1)} stroke-dasharray="{px(4)} {px(3)}" pointer-events="none" />
        {/if}
      {/each}
      {#if multiBox}
        <rect x={multiBox.x0} y={multiBox.y0} width={multiBox.x1 - multiBox.x0} height={multiBox.y1 - multiBox.y0} fill="none" stroke="var(--accent)" stroke-width={px(1.2)} pointer-events="none" />
      {/if}
      {#if single}
        <g transform="translate({single.x} {single.y}) rotate({single.rot} {single.w / 2} {single.h / 2})">
          <rect width={single.w} height={single.h} fill="none" stroke={single.locked ? "var(--text-3)" : "var(--accent)"} stroke-width={px(1.2)} pointer-events="none" />
          {#if !single.locked}
            <line x1={single.w / 2} y1="0" x2={single.w / 2} y2={-px(26)} stroke="var(--accent)" stroke-width={px(1.2)} />
            <circle data-handle="rot" cx={single.w / 2} cy={-px(26)} r={px(5.5)} fill="var(--panel)" stroke="var(--accent)" stroke-width={px(1.5)} style:cursor="grab" />
            {#each HANDLES as [hx, hy]}
              <rect
                data-handle="{hx},{hy}"
                x={((hx + 1) / 2) * single.w - px(4.5)}
                y={((hy + 1) / 2) * single.h - px(4.5)}
                width={px(9)}
                height={px(9)}
                rx={px(2)}
                fill="var(--panel)"
                stroke="var(--accent)"
                stroke-width={px(1.5)}
                style:cursor={hx === 0 ? "ns-resize" : hy === 0 ? "ew-resize" : hx === hy ? "nwse-resize" : "nesw-resize"}
              />
            {/each}
          {/if}
        </g>
      {/if}

      <!-- Marquee / drawing preview -->
      {#if drag?.kind === "marquee" || drag?.kind === "draw"}
        {@const x = Math.min(drag.start.x, drag.cur.x)}
        {@const y = Math.min(drag.start.y, drag.cur.y)}
        {@const w = Math.abs(drag.cur.x - drag.start.x)}
        {@const h = Math.abs(drag.cur.y - drag.start.y)}
        {#if drag.kind === "marquee"}
          <rect {x} {y} width={w} height={h} fill="var(--accent-soft)" stroke="var(--accent)" stroke-width={px(1)} pointer-events="none" />
        {:else if drag.shape === "rect"}
          <rect {x} {y} width={w} height={h} fill="none" stroke="var(--cut)" stroke-width={px(1.4)} pointer-events="none" />
        {:else}
          <ellipse cx={x + w / 2} cy={y + h / 2} rx={w / 2} ry={h / 2} fill="none" stroke="var(--cut)" stroke-width={px(1.4)} pointer-events="none" />
        {/if}
      {/if}
    </g>

    <!-- Rulers (screen space) -->
    <g class="ruler" pointer-events="none">
      <rect x="0" y="0" width={editor.viewport.w} height={RULER} fill="var(--ruler)" />
      <rect x="0" y="0" width={RULER} height={editor.viewport.h} fill="var(--ruler)" />
      {#each ticks(editor.viewport.w, editor.view.panX) as v}
        {@const sx = editor.view.panX + v * z}
        {#if sx > RULER}
          <line x1={sx} y1={RULER - 8} x2={sx} y2={RULER} stroke="var(--border-strong)" />
          <text x={sx + 3} y="11">{v}</text>
          <line x1={sx + (step * z) / 2} y1={RULER - 4} x2={sx + (step * z) / 2} y2={RULER} stroke="var(--border)" />
        {/if}
      {/each}
      {#each ticks(editor.viewport.h, editor.view.panY) as v}
        {@const sy = editor.view.panY + v * z}
        {#if sy > RULER}
          <line x1={RULER - 8} y1={sy} x2={RULER} y2={sy} stroke="var(--border-strong)" />
          <text x="11" y={sy + 3} transform="rotate(-90 11 {sy + 3})">{v}</text>
          <line x1={RULER - 4} y1={sy + (step * z) / 2} x2={RULER} y2={sy + (step * z) / 2} stroke="var(--border)" />
        {/if}
      {/each}
      {#if editor.cursor}
        <line x1={editor.view.panX + editor.cursor.x * z} y1="0" x2={editor.view.panX + editor.cursor.x * z} y2={RULER} stroke="var(--accent)" />
        <line x1="0" y1={editor.view.panY + editor.cursor.y * z} x2={RULER} y2={editor.view.panY + editor.cursor.y * z} stroke="var(--accent)" />
      {/if}
      <line x1="0" y1={RULER} x2={editor.viewport.w} y2={RULER} stroke="var(--border)" />
      <line x1={RULER} y1="0" x2={RULER} y2={editor.viewport.h} stroke="var(--border)" />
      <rect x="0" y="0" width={RULER} height={RULER} fill="var(--ruler)" />
      <text x="4" y="14" class="unit">mm</text>
    </g>
  </svg>

  {#if editor.picking}
    <div class="hint">Klicke auf eine Farbe im Bild · <kbd>Esc</kbd> zum Abbrechen</div>
  {:else if nodes.objId}
    <div class="hint">Knoten ziehen · Doppelklick auf die Linie = neuer Knoten · <kbd>Entf</kbd> löscht · <kbd>Esc</kbd> fertig</div>
  {:else if perspective.objId}
    {#if perspective.custom}
      <div class="hint">Punkte auf die Ecken des schwarzen Rahmens ziehen · gefüllter Punkt an die Ecke beim großen Quadrat</div>
    {:else}
      <div class="hint">Die vier Punkte auf die Ecken des Blatts ziehen · rechts „Entzerren“</div>
    {/if}
  {/if}
</div>

<style>
  .wrap {
    position: relative;
    flex: 1;
    min-width: 0;
    overflow: hidden;
    background: var(--canvas);
  }
  svg {
    display: block;
    width: 100%;
    height: 100%;
    touch-action: none;
  }
  .ruler text {
    font-size: 9.5px;
    fill: var(--ruler-text);
    font-variant-numeric: tabular-nums;
  }
  .ruler .unit {
    font-size: 8.5px;
  }
  .hint {
    position: absolute;
    top: 34px;
    left: 50%;
    transform: translateX(-50%);
    padding: 6px 12px;
    border-radius: 999px;
    background: var(--panel);
    box-shadow: var(--shadow);
    color: var(--text-2);
  }
  kbd {
    font: inherit;
    font-size: 11px;
    padding: 0 4px;
    border: 1px solid var(--border);
    border-radius: 4px;
  }
</style>
