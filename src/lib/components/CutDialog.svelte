<script lang="ts">
  import { untrack } from "svelte";
  import { api } from "../api";
  import { editor, MAT_LABELS, matSize, regBracketPath } from "../editor.svelte";
  import { aabb, fmt, uid } from "../geometry";
  import { PRESETS, lastMaterialId, loadCustom, rememberMaterial, saveCustom, testGrids, type Material, type TestGrid } from "../materials";
  import { matKindFor, pathMatrix, plotter } from "../plotter.svelte";
  import type { CutPlan, CutSettings, MatKind, PathInput, PathObj, PathOptions } from "../types";
  import Icon from "./Icon.svelte";
  import Slider from "./Slider.svelte";
  import Toggle from "./Toggle.svelte";
  import Segmented from "./Segmented.svelte";
  import { locale, t, tMsg } from "../i18n.svelte";
  import { ui } from "../ui.svelte";

  let custom = $state<Material[]>(loadCustom());
  const all = $derived([...PRESETS, ...custom]);
  const findMat = (id: string) => [...PRESETS, ...loadCustom()].find((p) => p.id === id);

  // Every cut colour is its own pass with its own material. `m` / `materialId`
  // are the working copy of the active colour; the others live in `byColor`.
  let byColor = $state<Record<string, { materialId: string; m: Material }>>({});
  let active = $state<string>("");
  /** Project material (new-project assistant), else the last one used. */
  const preferred = () => (editor.material && findMat(editor.material) ? editor.material : lastMaterialId());
  let materialId = $state(preferred());
  let m = $state<Material>(structuredClone(findMat(preferred()) ?? PRESETS[0]));
  let mirror = $state(!!findMat(preferred())?.mirror);
  /** Mirror the whole sheet, or every element around its own centre line. */
  let mirrorMode = $state<"sheet" | "each">(loadMirrorMode());
  function loadMirrorMode(): "sheet" | "each" {
    try {
      return localStorage.getItem("schnittwerk.mirrorMode") === "each" ? "each" : "sheet";
    } catch {
      return "sheet";
    }
  }
  function setMirrorMode(v: "sheet" | "each") {
    mirrorMode = v;
    try {
      localStorage.setItem("schnittwerk.mirrorMode", v);
    } catch {}
  }
  let mat = $state<MatKind>(matKindFor(editor.mat));
  /** Roll media: push the media out behind the design at the end. */
  let feedOut = $state(true);
  let onlySelection = $state(editor.selected.some((o) => o.type === "path"));
  let optimize = $state(true);
  let testX = $state(5);
  let testY = $state(5);
  let plans = $state<CutPlan[]>([]);
  let planError = $state<string | null>(null);

  /** `remember` = the user chose it (automatic defaults are not stored). */
  function pickMaterial(id: string, remember = true) {
    const found = all.find((x) => x.id === id);
    if (!found) return;
    materialId = id;
    m = structuredClone($state.snapshot(found) as Material);
    if (m.mirror) mirror = true;
    if (remember) rememberMaterial(id);
  }

  function selectColor(c: string) {
    if (c === active) return;
    if (active) byColor[active] = { materialId, m: $state.snapshot(m) as Material };
    active = c;
    const entry = byColor[c];
    if (entry) {
      materialId = entry.materialId;
      m = $state.snapshot(entry.m) as Material;
    }
  }

  const modified = $derived.by(() => {
    const base = all.find((x) => x.id === materialId);
    return !!base && JSON.stringify({ ...base, custom: undefined }) !== JSON.stringify({ ...m, custom: undefined });
  });

  let naming = $state<string | null>(null);

  function saveAsMaterial() {
    const name = naming?.trim();
    if (!name) return;
    const entry: Material = { ...$state.snapshot(m), id: "c-" + uid(), name, custom: true };
    custom = [...custom, entry];
    saveCustom(custom);
    materialId = entry.id;
    m = structuredClone(entry);
    rememberMaterial(entry.id);
    naming = null;
    editor.toast(t("Material „{name}“ gespeichert", { name }), "ok");
  }

  function deleteMaterial() {
    custom = custom.filter((c) => c.id !== materialId);
    saveCustom(custom);
    pickMaterial(PRESETS[0].id);
  }

  // ---------- job data ----------
  const cutObjects = $derived.by((): PathObj[] => {
    const paths = editor.objects.filter((o): o is PathObj => o.type === "path" && o.visible && o.cut !== false);
    if (onlySelection) {
      const sel = paths.filter((o) => editor.selection.includes(o.id));
      if (sel.length) return sel;
    }
    return paths;
  });

  const colors = $derived([...new Set(cutObjects.map((o) => o.color.toLowerCase()))]);

  /** Centre line (x) of each object's unit: its group, or the object itself. */
  const unitCenter = $derived.by(() => {
    const units = new Map<string, PathObj[]>();
    for (const o of cutObjects) units.set(o.group ?? o.id, [...(units.get(o.group ?? o.id) ?? []), o]);
    const out = new Map<string, number>();
    for (const objs of units.values()) {
      const b = aabb(objs)!;
      for (const o of objs) out.set(o.id, (b.x0 + b.x1) / 2);
    }
    return out;
  });

  /** `mirrorWidth` for pathMatrix: x → w − x mirrors around w / 2. */
  function mirrorAxis(o: PathObj): number | null {
    if (!mirror) return null;
    return mirrorMode === "sheet" ? editor.mat.w : 2 * (unitCenter.get(o.id) ?? o.x + o.w / 2);
  }

  /** Blue = through-cut, green = pen, otherwise the remembered material. */
  function defaultMaterialFor(color: string, index: number): string {
    if (color === "#3e63dd") return "sticker-through";
    if (color === "#30a46c") return "pen";
    if (color === "#e5484d") {
      // Red is the kiss-cut: never a through-cut or pen preset.
      const last = preferred();
      return ["sticker-through", "pen", "cardstock", "magnet"].includes(last) ? "sticker-matt" : last;
    }
    return index === 0 ? preferred() : "sticker-through";
  }

  // New colours get a material: the first the remembered one, further ones
  // a through-cut preset (typical: kiss-cut stickers + through-cut frame).
  $effect(() => {
    for (const [i, c] of colors.entries()) {
      if (c === active || byColor[c]) continue;
      if (!active) {
        active = c;
        const id = defaultMaterialFor(c, 0);
        if (id !== materialId) pickMaterial(id, false);
        continue;
      }
      const preset = findMat(defaultMaterialFor(c, i)) ?? PRESETS[0];
      byColor[c] = { materialId: preset.id, m: structuredClone($state.snapshot(preset) as Material) };
    }
    if (active && !colors.includes(active) && colors.length) selectColor(colors[0]);
  });

  /** Material of a colour (live working copy for the active one). */
  const matOf = (c: string): Material => (c === active ? m : (byColor[c]?.m ?? m));

  /** Lighter passes first so the sheet holds together while kiss-cutting. */
  const severity = (x: Material) => (x.tool === "pen" ? -1 : x.depth * 10 + x.pressure + x.passes * 5);
  /** Chosen material of a colour as stored (not the live slider values), so
   *  adjusting a slider never reshuffles the passes under the cursor. */
  const baseOf = (c: string): Material => {
    const id = c === active ? materialId : byColor[c]?.materialId;
    return all.find((x) => x.id === id) ?? matOf(c);
  };
  const bySeverity = (list: string[]) => [...list].sort((a, b) => severity(baseOf(a)) - severity(baseOf(b)));

  // Cut order of the colours. Sorted (lighter first) only when colours come
  // or go – picking a material never reshuffles the tabs, so the one being
  // edited stays where it is. The user can move passes by hand.
  let manualOrder = $state<string[]>([]);
  $effect(() => {
    const cur = colors;
    const prev = untrack(() => manualOrder);
    if (prev.length === cur.length && cur.every((c) => prev.includes(c))) return;
    manualOrder = untrack(() => bySeverity(cur));
  });
  const order = $derived([...manualOrder.filter((c) => colors.includes(c)), ...colors.filter((c) => !manualOrder.includes(c))]);

  function moveColor(c: string, dir: -1 | 1) {
    const list = [...order];
    const i = list.indexOf(c);
    const j = i + dir;
    if (i < 0 || j < 0 || j >= list.length) return;
    [list[i], list[j]] = [list[j], list[i]];
    manualOrder = list;
  }

  /** A heavier pass comes before a lighter one (sheet may fall apart early). */
  const misordered = $derived(order.some((c, i) => i > 0 && severity(baseOf(order[i - 1])) > severity(baseOf(c))));

  // ---------- tiles (designs larger than the cutting area) ----------
  let tileOn = $state(true);
  let tileOverlap = $state(5);
  let tileIdx = $state(0);
  let tilesDone = $state<number[]>([]);
  /** Tile being cut right now (marked done when the job finishes). */
  let cuttingTile: number | null = null;

  /** Cuttable area of the chosen mat / roll (without print & cut). */
  const cutArea = $derived(matSize(mat) ?? { w: plotter.status.widthMm ?? 203.2, h: editor.mat.h });
  /** The chosen mat is wider than the connected cutter (12″ mat in a Portrait). */
  const matTooWide = $derived(!!plotter.status.widthMm && cutArea.w > plotter.status.widthMm + 1);

  /** Bounding box of the design in cut coordinates (after mirroring). */
  const designBox = $derived.by(() => {
    const b = aabb(cutObjects);
    if (!b) return null;
    // Mirrored in place, every element keeps its box.
    return mirror && mirrorMode === "sheet" ? { x0: editor.mat.w - b.x1, y0: b.y0, x1: editor.mat.w - b.x0, y1: b.y1 } : b;
  });

  /** Does the design need tiling at all (bigger than / outside the area)? */
  const tooBig = $derived(
    !editor.regmarks && !!designBox && (designBox.x0 < -0.05 || designBox.y0 < -0.05 || designBox.x1 > cutArea.w + 0.05 || designBox.y1 > cutArea.h + 0.05),
  );

  const tiling = $derived.by(() => {
    if (!tooBig || !tileOn || !designBox) return null;
    const { w: tw, h: th } = cutArea;
    const ov = Math.max(0, Math.min(tileOverlap, Math.min(tw, th) / 4));
    const W = designBox.x1 - designBox.x0, H = designBox.y1 - designBox.y0;
    const cols = W <= tw ? 1 : Math.ceil((W - ov) / (tw - ov));
    const rows = H <= th ? 1 : Math.ceil((H - ov) / (th - ov));
    const tiles: { x: number; y: number; w: number; h: number; col: number; row: number }[] = [];
    for (let r = 0; r < rows; r++)
      for (let c = 0; c < cols; c++) tiles.push({ x: designBox.x0 + c * (tw - ov), y: designBox.y0 + r * (th - ov), w: tw, h: th, col: c, row: r });
    return { cols, rows, tiles, box: designBox };
  });
  const tile = $derived(tiling ? tiling.tiles[Math.min(tileIdx, tiling.tiles.length - 1)] : null);

  // A different layout starts the tile series again.
  let tileKey = "";
  $effect(() => {
    const k = tiling ? `${tiling.cols}x${tiling.rows}:${fmt(tiling.box.x0)}:${fmt(tiling.box.y0)}:${tileOverlap}` : "";
    if (k !== tileKey) {
      tileKey = k;
      untrack(() => {
        tileIdx = 0;
        tilesDone = [];
      });
    }
  });

  // Finished tile: mark it and move on to the next open one.
  $effect(() => {
    const r = plotter.lastResult;
    if (!r) return;
    untrack(() => {
      const doneTile = cuttingTile;
      cuttingTile = null;
      if (doneTile === null || r.status !== "done" || !tiling) return;
      if (!tilesDone.includes(doneTile)) tilesDone = [...tilesDone, doneTile];
      const next = tiling.tiles.findIndex((_, i) => !tilesDone.includes(i));
      if (next >= 0) {
        tileIdx = next;
        editor.toast(t("Kachel {a} fertig – neues Material einlegen, dann Kachel {b} schneiden.", { a: doneTile + 1, b: next + 1 }), "ok");
      } else {
        editor.toast("Alle Kacheln geschnitten ✓", "ok");
      }
    });
  });

  const groups = $derived(
    order.map((c) => {
      const mm = matOf(c);
      const objs = cutObjects.filter((o) => o.color.toLowerCase() === c);
      const shift = editor.regmarks ? plotter.calib : tile ? { dx: -tile.x, dy: -tile.y } : undefined;
      return {
        color: c,
        name: mm.name,
        paths: objs.map((o) => ({ d: o.d, matrix: pathMatrix(o, mirrorAxis(o), shift) })) as PathInput[],
        options: { overcut: mm.tool === "pen" ? 0 : mm.overcut, passes: mm.passes, optimize, clip: tile ? [0, 0, tile.w, tile.h] : null } as PathOptions,
        settings: {
          tool: mm.tool,
          pressure: mm.pressure,
          speed: mm.speed,
          depth: mm.tool === "blade" ? mm.depth : null,
          mat,
          bladeOffset: 0.9,
          liftCorners: false,
        } as CutSettings,
      };
    }),
  );
  /** Settings of the active colour (test cut, mark test). */
  const settings = $derived(groups.find((g) => g.color === active)?.settings ?? groups[0]?.settings);

  let seq = 0;
  $effect(() => {
    const req = $state.snapshot(groups.map((g) => ({ paths: g.paths, options: g.options })));
    const my = ++seq;
    if (!req.length) {
      plans = [];
      return;
    }
    const t = setTimeout(async () => {
      try {
        const p = await api.cutPreview(req);
        if (my === seq) {
          plans = p;
          planError = null;
        }
      } catch (e) {
        if (my === seq) planError = String(e);
      }
    }, 120);
    return () => clearTimeout(t);
  });

  const totals = $derived({
    paths: plans.reduce((n, p) => n + p.strokes.length, 0),
    cut: plans.reduce((n, p) => n + p.cutLength, 0),
    travel: plans.reduce((n, p) => n + p.travelLength, 0),
  });

  const reg = $derived(editor.regmarks);

  /** Adds a calibration sheet: printed 10 mm squares inside 14 mm cut frames. */
  function addCalibrationSheet() {
    const r = editor.regmarks;
    if (!r) return;
    const spots: [number, number][] = [
      [r.originX + r.width / 2, r.originY + 30],
      [r.originX + 35, r.originY + r.length / 2],
      [r.originX + r.width - 35, r.originY + r.length / 2],
      [r.originX + r.width / 2, r.originY + r.length - 30],
    ];
    const g = uid();
    const objs: PathObj[] = spots.flatMap(([cx, cy]) => {
      const sq = (s: number, extra: Partial<PathObj>): PathObj => ({
        id: uid(), type: "path", name: t("Kalibrierung"), d: `M0 0H${s}V${s}H0Z`, bbox: [0, 0, s, s], color: "#e5484d", fill: false,
        x: cx - s / 2, y: cy - s / 2, w: s, h: s, rot: 0, flipX: false, flipY: false, visible: true, locked: false, group: g, ...extra,
      });
      return [sq(10, { printFill: "#000000", cut: false, color: "#8b919c" }), sq(14, {})];
    });
    editor.add(objs);
    editor.toast("Testbogen angelegt: drucken, schneiden, dann den weißen Rand um die Quadrate prüfen.", "ok");
  }
  const area = $derived(reg ? { w: editor.mat.w, h: editor.mat.h } : cutArea);
  const limits = $derived(reg ? { x0: reg.originX, y0: reg.originY, x1: reg.originX + reg.width, y1: reg.originY + reg.length } : { x0: 0, y0: 0, x1: area.w, y1: area.h });
  const outside = $derived(
    plans.some(
      (plan) =>
        plan.strokes.length > 0 &&
        (plan.bbox[0] < limits.x0 - 0.05 || plan.bbox[1] < limits.y0 - 0.05 || plan.bbox[0] + plan.bbox[2] > limits.x1 + 0.05 || plan.bbox[1] + plan.bbox[3] > limits.y1 + 0.05),
    ),
  );

  // ---------- actions ----------
  const ready = $derived(plotter.status.connected && (plotter.status.state === "ready" || plotter.status.state === "unknown"));

  /** Roll media: where to push the media to after the cut (mm). */
  const feed = $derived.by(() => {
    if (reg || mat !== "none" || !feedOut) return null;
    const end = Math.max(0, ...plans.filter((p) => p.strokes.length).map((p) => p.bbox[1] + p.bbox[3]));
    return end > 0 ? end + 15 : null;
  });

  /** Length cut with the blade (wear counter). */
  const bladeMm = $derived(plans.reduce((n, p, i) => n + (groups[i]?.settings.tool === "blade" ? p.cutLength : 0), 0));

  // ---------- time estimate ----------
  /**
   * Rough cut time in seconds before the learned correction: speed n ≈ n cm/s
   * while cutting (slower in corners), lifting the blade per path, AutoBlade
   * adjustment per pass and the registration mark search.
   */
  const rawEstimate = $derived.by(() => {
    if (!plans.length) return 0;
    let s = 4;
    if (reg) s += 35;
    plans.forEach((p, i) => {
      const g = groups[i];
      if (!g || !p.strokes.length) return;
      const v = Math.max(1, g.settings.speed) * 10 * 0.7;
      s += p.cutLength / v + p.travelLength / Math.max(v, 100) + p.strokes.length * 0.3 + (g.settings.depth !== null ? 5 : 1);
    });
    return s;
  });
  const estimate = $derived(rawEstimate * plotter.timeFactor);

  function fmtDuration(sec: number): string {
    if (sec < 60) return t("< 1 Min");
    const min = Math.round(sec / 60);
    return min < 60 ? t("{m} Min", { m: min }) : t("{h} Std {m} Min", { h: Math.floor(min / 60), m: min % 60 });
  }

  /** Clock for the remaining time while cutting. */
  let now = $state(Date.now());
  $effect(() => {
    if (!plotter.busy) return;
    const t = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(t);
  });
  const remaining = $derived.by(() => {
    const t = plotter.timing;
    if (!t) return null;
    const pausedNow = t.pausedAt ? now - t.pausedAt : 0;
    const elapsed = (now - t.start - t.paused - pausedNow) / 1000;
    return Math.max(0, t.estimate * plotter.timeFactor - elapsed);
  });

  // ---------- continue after an interruption ----------
  /** Short hash of the cut data – resuming only makes sense for the same job. */
  function hash(str: string): string {
    let h = 0x811c9dc5;
    for (let i = 0; i < str.length; i++) h = Math.imul(h ^ str.charCodeAt(i), 0x01000193);
    return (h >>> 0).toString(36) + str.length.toString(36);
  }
  const jobGroups = $derived(groups.filter((g) => g.paths.length > 0));
  const jobKey = $derived(hash(JSON.stringify([jobGroups.map((g) => [g.paths, g.options]), reg, mirror])));
  const resume = $derived(plotter.resume && plotter.resume.key === jobKey ? plotter.resume : null);
  /** Paths to repeat before the interruption point (adjustable). */
  let back = $state(0);
  $effect(() => {
    void resume?.point;
    back = 0;
  });
  const resumeFrom = $derived(resume ? Math.max(0, resume.point.stroke - back) : 0);

  async function startCut(fromResume = false) {
    try {
      // Empty groups are skipped by the backend too; dropping them here keeps
      // the pass numbers in the progress display in step with the device.
      const job = $state.snapshot(jobGroups) as typeof groups;
      const point = fromResume && resume ? { pass: resume.point.pass, stroke: resumeFrom } : null;
      cuttingTile = tiling ? tileIdx : null;
      plotter.begin(
        // Resumed: the progress shows the remaining passes.
        job.slice(point?.pass ?? 0).map((g) => ({ color: g.color, name: g.name })),
        point ? 0 : bladeMm,
        point ? 0 : rawEstimate,
        jobKey,
      );
      const r = await api.cutStart(
        job.map((g) => ({ paths: g.paths, options: g.options, settings: g.settings })),
        reg ? { ...reg } : null,
        feed,
        point,
      );
      if (r.clamped > 0) editor.toast(t("{n} Punkte lagen außerhalb und wurden an den Rand gesetzt.", { n: r.clamped }), "error");
    } catch (e) {
      cuttingTile = null;
      plotter.phase = "idle";
      editor.error(e);
    }
  }

  async function testMarks() {
    if (!reg) return;
    try {
      plotter.begin();
      plotter.regtest = true;
      await api.cutRegtest({ ...reg }, $state.snapshot(settings ?? groups[0]?.settings) as CutSettings);
    } catch (e) {
      plotter.phase = "idle";
      editor.error(e);
    }
  }

  async function testCut() {
    try {
      plotter.begin();
      testRunning = true;
      testFeedback = false;
      const st = settings ?? {
        tool: m.tool, pressure: m.pressure, speed: m.speed, depth: m.tool === "blade" ? m.depth : null, mat, bladeOffset: 0.9, liftCorners: false,
      };
      await api.cutTest(testX, testY, $state.snapshot(st) as CutSettings);
    } catch (e) {
      testRunning = false;
      plotter.phase = "idle";
      editor.error(e);
    }
  }

  // ---------- test series: a grid of blade depth × force ----------
  let seriesOpen = $state(false);
  /** Values as typed ("1, 2, 3"); rows = depths, columns = forces. */
  let depthText = $state("");
  let pressureText = $state("");
  /** Grid of the last series cut – choose the cleanest square. */
  let seriesCut = $state<{ depths: number[]; pressures: number[] } | null>(null);

  const MAX_AXIS = 6;
  const MAX_SQUARES = 30;
  function parseList(text: string, lo: number, hi: number): number[] {
    const vals = text
      .split(/[\s,;]+/)
      .map((v) => Math.round(parseFloat(v.replace(",", "."))))
      .filter((v) => Number.isFinite(v))
      .map((v) => Math.min(hi, Math.max(lo, v)));
    return [...new Set(vals)].sort((x, y) => x - y).slice(0, MAX_AXIS);
  }
  const isPen = $derived(m.tool === "pen");
  const seriesDepths = $derived(isPen ? [m.depth] : parseList(depthText, 1, 10));
  const seriesPressures = $derived(parseList(pressureText, 1, 33));
  const seriesCount = $derived(seriesDepths.length * seriesPressures.length);
  const seriesCols = $derived(Math.max(1, seriesPressures.length));
  const seriesRows = $derived(Math.max(1, seriesDepths.length));
  const seriesFits = $derived(testX + seriesCols * 13 - 3 <= area.w && testY + seriesRows * 13 - 3 <= area.h);
  const seriesValid = $derived(seriesCount > 0 && seriesCount <= MAX_SQUARES);

  /** Suggested grids for the active material. */
  const grids = $derived(testGrids(m, materialId));
  const same = (a: number[], b: number[]) => a.length === b.length && a.every((v, i) => v === b[i]);
  function applyGrid(g: TestGrid) {
    depthText = g.depths.join(", ");
    pressureText = g.pressures.join(", ");
    seriesCut = null;
    untrack(ensureFreeTestSpot);
  }

  function openSeries() {
    seriesOpen = !seriesOpen;
    testFeedback = false;
    if (seriesOpen) applyGrid(grids[0]);
    else untrack(ensureFreeTestSpot);
  }

  async function cutSeries() {
    const base = $state.snapshot(
      settings ?? { tool: m.tool, pressure: m.pressure, speed: m.speed, depth: m.tool === "blade" ? m.depth : null, mat, bladeOffset: 0.9, liftCorners: false },
    ) as CutSettings;
    const depths = [...seriesDepths], pressures = [...seriesPressures];
    // Row by row: each row one depth (AutoBlade adjusts once), forces rising.
    const list = depths.flatMap((d) => pressures.map((p) => ({ ...base, pressure: p, depth: base.tool === "blade" ? d : null })));
    try {
      plotter.begin();
      seriesCut = { depths, pressures };
      await api.cutTestSeries(testX, testY, list, pressures.length);
    } catch (e) {
      plotter.phase = "idle";
      seriesCut = null;
      editor.error(e);
    }
  }

  /** Takes the chosen combination into the active material. */
  function chooseSeries(d: number, p: number) {
    if (!seriesCut) return;
    m.pressure = p;
    if (m.tool === "blade") m.depth = d;
    editor.toast(
      m.tool === "blade"
        ? t("Messertiefe {d} und Druck {p} übernommen – zum Behalten das Material speichern.", { d, p })
        : t("Druck {v} übernommen – zum Behalten das Material speichern.", { v: p }),
      "ok",
    );
    const w = seriesCut.pressures.length * 13 - 3;
    seriesCut = null;
    seriesOpen = false;
    const f = findFree(testX + w + 3, testY) ?? findFree();
    if (f) [testX, testY] = f;
  }

  // ---------- wizard ----------
  const STEPS = [
    { n: 1, label: "Vorbereiten" },
    { n: 2, label: "Material" },
    { n: 3, label: "Testschnitt" },
    { n: 4, label: "Schneiden" },
  ] as const;
  type Step = (typeof STEPS)[number]["n"];
  /** An interrupted job continues right away; otherwise start at the top. */
  let step = $state<Step>(untrack(() => (resume ? 4 : 1)));
  /** Material values open (most of the time the preset fits). */
  let tune = $state(false);

  function go(n: Step) {
    if (plotter.busy) return;
    step = n;
    if (n === 3) untrack(ensureFreeTestSpot);
  }

  // ---------- test position (picked in the preview) ----------
  const TEST = 10;
  /** Width of what the next test cuts: one square or the whole series. */
  const seriesShown = $derived(seriesOpen && !seriesCut && seriesValid);
  const testW = $derived(seriesShown ? seriesCols * 13 - 3 : TEST);
  const testH = $derived(seriesShown ? seriesRows * 13 - 3 : TEST);
  /** Boxes of everything that will be cut, to keep the test off the design. */
  const strokeBoxes = $derived(
    plans.flatMap((plan) =>
      plan.strokes.map((st) => {
        let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
        for (const [x, y] of st) {
          x0 = Math.min(x0, x); y0 = Math.min(y0, y); x1 = Math.max(x1, x); y1 = Math.max(y1, y);
        }
        return { x0, y0, x1, y1 };
      }),
    ),
  );
  /** Everything else on the sheet (not cut now, but in the way: other
   *  stickers, printed pictures). Not for tiles – they move the design. */
  const sheetBoxes = $derived.by(() => {
    if (tile) return [];
    return editor.objects
      .filter((o) => o.visible && (o.type === "image" ? !!reg : o.cut !== false))
      .map((o) => aabb([o])!)
      .map((b) => (mirror && mirrorMode === "sheet" ? { x0: editor.mat.w - b.x1, y0: b.y0, x1: editor.mat.w - b.x0, y1: b.y1 } : b));
  });
  function blocked(x: number, y: number, w: number, h = TEST, margin = 2): boolean {
    if (x < 0 || y < 0 || x + w > area.w || y + h > area.h) return true;
    const hit = (b: { x0: number; y0: number; x1: number; y1: number }) => x < b.x1 + margin && x + w > b.x0 - margin && y < b.y1 + margin && y + h > b.y0 - margin;
    if (strokeBoxes.some(hit) || sheetBoxes.some(hit)) return true;
    return editor.regZones().some(([zx, zy, zw, zh]) => hit({ x0: zx, y0: zy, x1: zx + zw, y1: zy + zh }));
  }
  const testOnDesign = $derived(blocked(testX, testY, testW, testH));

  /** First free spot (row by row from the top left), starting after (x, y). */
  function findFree(fromX = 0, fromY = 0, w = testW, h = testH): [number, number] | null {
    const stepMm = 5;
    for (let y = Math.max(3, fromY); y + h <= area.h; y += stepMm) {
      for (let x = y === Math.max(3, fromY) ? Math.max(3, fromX) : 3; x + w <= area.w; x += stepMm) {
        if (!blocked(x, y, w, h)) return [x, y];
      }
    }
    return null;
  }
  function ensureFreeTestSpot() {
    if (!blocked(testX, testY, testW, testH)) return;
    const f = findFree();
    if (f) [testX, testY] = f;
  }
  /** After a test: the next one goes next to it, onto fresh material. */
  function moveTestOn() {
    const f = findFree(testX + testW + 3, testY) ?? findFree();
    if (f) [testX, testY] = f;
  }

  let previewSvg = $state<SVGSVGElement>();
  function placeTest(e: PointerEvent) {
    if (step !== 3 || plotter.busy || !previewSvg) return;
    if (e.type === "pointermove" && !(e.buttons & 1)) return;
    const ctm = previewSvg.getScreenCTM();
    if (!ctm) return;
    const pt = new DOMPoint(e.clientX, e.clientY).matrixTransform(ctm.inverse());
    // The click is the centre of the square (the first one of a series).
    testX = Math.round(Math.max(0, Math.min(area.w - testW, pt.x - TEST / 2)) * 2) / 2;
    testY = Math.round(Math.max(0, Math.min(area.h - testH, pt.y - TEST / 2)) * 2) / 2;
    testFeedback = false;
  }

  // ---------- after a test cut: how did it go? ----------
  let testRunning = false;
  let testFeedback = $state(false);
  $effect(() => {
    const r = plotter.lastResult;
    if (!r) return;
    untrack(() => {
      if (!testRunning) return;
      testRunning = false;
      if (r.status === "done") testFeedback = true;
    });
  });

  /** "Too light" / "too deep": change the active material one notch, test again. */
  function adjustTest(dir: 1 | -1) {
    const before = { depth: m.depth, pressure: m.pressure };
    if (m.tool === "blade" && m.depth + dir >= 1 && m.depth + dir <= 10) m.depth += dir;
    else m.pressure = Math.max(1, Math.min(33, m.pressure + 2 * dir));
    const msg =
      m.depth !== before.depth
        ? t("Messertiefe {a} → {b} – noch einmal testen.", { a: before.depth, b: m.depth })
        : t("Druck {a} → {b} – noch einmal testen.", { a: before.pressure, b: m.pressure });
    editor.toast(msg, "ok");
    testFeedback = false;
    moveTestOn();
  }

  function testGood() {
    testFeedback = false;
    moveTestOn();
    go(4);
  }

  function close() {
    if (!plotter.busy) plotter.dialogOpen = false;
  }

  /** Colour of the pass being cut right now (multi-pass jobs), else null. */
  const runningColor = $derived(plotter.busy && plotter.job.length > 1 ? (plotter.job[plotter.pass]?.color ?? null) : null);
  const allPen = $derived(groups.length > 0 && groups.every((g) => g.settings.tool === "pen"));

  const progressText = $derived.by(() => {
    if (plotter.registering) return t("Suche Passermarken …");
    if (plotter.phase === "paused") return t("Pausiert");
    if (plotter.phase === "cancelling") return t("Breche ab …");
    const n = plotter.job.length;
    const step = n > 1 ? `${t("Schritt {a}/{b}", { a: Math.min(plotter.pass + 1, n), b: n })} (${t(plotter.job[plotter.pass]?.name ?? "")}) · ` : "";
    if (plotter.draining) {
      const last = plotter.pass >= n - 1;
      return `${step}${last ? t("schneidet die letzten Linien …") : t("schneidet den Rest dieses Schritts …")}`;
    }
    const left = remaining !== null && remaining > 0 ? ` · ${t("noch ca. {d}", { d: fmtDuration(remaining) })}` : "";
    return `${step}${pct} %${left}`;
  });

  const pct = $derived(plotter.progress.total ? Math.round((plotter.progress.done / plotter.progress.total) * 100) : 0);

  const STATE_TEXT: Record<string, string> = {
    ready: "Bereit",
    moving: "In Bewegung",
    unloaded: "Keine Matte geladen",
    cutting: "Schneidet …",
    unknown: "Bereit",
    disconnected: "Nicht verbunden",
    error: "Fehler",
  };

  function strokeColor(i: number, n: number) {
    const t = n <= 1 ? 0 : i / (n - 1);
    return `hsl(${260 - t * 230} 85% 58%)`;
  }

  // ---------- simulation ----------
  /** Strokes with their length and where they start on the head's way (mm). */
  const simStrokes = $derived.by(() => {
    let pos = 0;
    let at: [number, number] = [reg ? reg.originX : 0, reg ? reg.originY : 0];
    return plans.map((plan) =>
      plan.strokes.map((st) => {
        const travel = Math.hypot(st[0][0] - at[0], st[0][1] - at[1]);
        pos += travel;
        const cum = [0];
        for (let k = 1; k < st.length; k++) cum.push(cum[k - 1] + Math.hypot(st[k][0] - st[k - 1][0], st[k][1] - st[k - 1][1]));
        const s = { start: pos, len: cum[cum.length - 1], travel, cum, pts: st, points: st.map((q) => q.join(",")).join(" "), from: at };
        pos += s.len;
        at = st[st.length - 1];
        return s;
      }),
    );
  });
  const simTotal = $derived.by(() => {
    const last = simStrokes.flat().at(-1);
    return last ? last.start + last.len : 0;
  });

  let sim = $state<{ pos: number } | null>(null);
  /** Speed factor; 1× follows the estimated cut time roughly. */
  let simSpeed = $state(5);
  let simFrame = 0;

  function toggleSim() {
    if (sim) return stopSim();
    if (!simTotal) return;
    sim = { pos: 0 };
    let last = performance.now();
    const step = (now: number) => {
      if (!sim) return;
      const dt = (now - last) / 1000;
      last = now;
      // 1×: the whole way in about the estimated time.
      const perSec = (simTotal / Math.max(5, estimate)) * simSpeed;
      sim.pos = Math.min(simTotal, sim.pos + perSec * dt);
      if (sim.pos >= simTotal) {
        simFrame = window.setTimeout(() => (sim = null), 1200) as unknown as number;
        return;
      }
      simFrame = requestAnimationFrame(step);
    };
    simFrame = requestAnimationFrame(step);
  }

  function stopSim() {
    cancelAnimationFrame(simFrame);
    clearTimeout(simFrame);
    sim = null;
  }

  // Another job or changed lines end the simulation.
  $effect(() => {
    void plans;
    if (plotter.busy) untrack(stopSim);
  });
  $effect(() => () => stopSim());

  /** Where the head is and whether the blade is down. */
  const simHead = $derived.by(() => {
    if (!sim) return null;
    const flat = simStrokes.flat();
    for (const s of flat) {
      if (sim.pos < s.start) {
        // Travelling towards this stroke.
        const k = s.travel ? 1 - (s.start - sim.pos) / s.travel : 1;
        return { x: s.from[0] + (s.pts[0][0] - s.from[0]) * k, y: s.from[1] + (s.pts[0][1] - s.from[1]) * k, down: false };
      }
      const d = sim.pos - s.start;
      if (d <= s.len) {
        let j = 1;
        while (j < s.cum.length - 1 && s.cum[j] < d) j++;
        const seg = s.cum[j] - s.cum[j - 1] || 1;
        const f = (d - s.cum[j - 1]) / seg;
        const a = s.pts[j - 1], b = s.pts[j];
        return { x: a[0] + (b[0] - a[0]) * f, y: a[1] + (b[1] - a[1]) * f, down: true };
      }
    }
    const last = flat.at(-1)!;
    const p = last.pts[last.pts.length - 1];
    return { x: p[0], y: p[1], down: false };
  });

  const fmtLen = (mm: number) => (mm >= 1000 ? `${fmt(mm / 1000, 2)} m` : `${fmt(mm / 10, 1)} cm`);

  /** Path of a cut object in the tile map (world coordinates, mirrored like the cut). */
  function mapTransform(o: PathObj) {
    const [a, b, c, d, e, f] = pathMatrix(o, mirrorAxis(o));
    return `matrix(${a} ${b} ${c} ${d} ${e} ${f})`;
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

<div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && close()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label={t("Schneiden")}>
    <header>
      <h2><Icon name="scissors" size={18} /> {t("Schneiden")}</h2>
      <button class="btn icon" onclick={close} disabled={plotter.busy} aria-label={t("Schließen")}><Icon name="x" /></button>
    </header>

    <div class="body">
      <!-- Preview -->
      <div class="preview">
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <svg
          viewBox="-6 -6 {area.w + 12} {area.h + 12}"
          preserveAspectRatio="xMidYMid meet"
          bind:this={previewSvg}
          class:picking={step === 3 && !plotter.busy}
          onpointerdown={placeTest}
          onpointermove={placeTest}
        >
          <rect x="0" y="0" width={area.w} height={area.h} rx="1" class="mat" class:paper={!!reg} />
          {#if reg}
            <rect x={reg.originX} y={reg.originY} width="5" height="5" fill="#000" />
            <path d={regBracketPath(reg)} fill="none" stroke="#000" stroke-width="0.5" stroke-linejoin="miter" />
          {/if}
          {#each Array.from({ length: Math.floor(area.w / 25.4) + 1 }) as _, i}
            <line x1={i * 25.4} y1="0" x2={i * 25.4} y2={area.h} class="grid" />
          {/each}
          {#each Array.from({ length: Math.floor(area.h / 25.4) + 1 }) as _, i}
            <line x1="0" y1={i * 25.4} x2={area.w} y2={i * 25.4} class="grid" />
          {/each}
          {#each plans as plan, gi}
            {@const multi = plans.length > 1}
            {#each plan.strokes as st, i}
              {@const prev =
                i > 0
                  ? plan.strokes[i - 1][plan.strokes[i - 1].length - 1]
                  : gi > 0 && plans[gi - 1].strokes.length
                    ? plans[gi - 1].strokes[plans[gi - 1].strokes.length - 1].at(-1)!
                    : [reg ? reg.originX : 0, reg ? reg.originY : 0]}
              {@const sk = simStrokes[gi]?.[i]}
              {@const drawn = sim && sk ? Math.min(sk.len, Math.max(0, sim.pos - sk.start)) : 0}
              {#if !sim || (sk && sim.pos >= sk.start - sk.travel)}
                <line x1={prev[0]} y1={prev[1]} x2={st[0][0]} y2={st[0][1]} class="travel" />
              {/if}
              {#if sim && sk}
                <!-- Simulation: faint outline, the cut part drawn on top -->
                <polyline points={sk.points} fill="none" stroke="var(--text-3)" opacity="0.18" stroke-width="0.4" stroke-linejoin="round" />
                {#if drawn > 0}
                  <polyline
                    points={sk.points}
                    fill="none"
                    stroke={multi ? groups[gi]?.color : strokeColor(i, plan.strokes.length)}
                    stroke-width="0.6"
                    stroke-linejoin="round"
                    stroke-dasharray="{sk.len} {sk.len}"
                    stroke-dashoffset={sk.len - drawn}
                  />
                {/if}
              {:else}
                <polyline
                  points={st.map((q) => q.join(",")).join(" ")}
                  fill="none"
                  stroke={multi ? groups[gi]?.color : strokeColor(i, plan.strokes.length)}
                  opacity={runningColor && groups[gi]?.color !== runningColor ? 0.25 : 1}
                  stroke-width="0.5"
                  stroke-linejoin="round"
                />
              {/if}
            {/each}
          {/each}
          {#if plans.length}
            <circle cx={reg ? reg.originX : 0} cy={reg ? reg.originY : 0} r="1.5" class="origin" />
          {/if}
          {#if step === 3}
            <!-- Where the test cut goes (one square, or the whole series) -->
            <g class="testmark" class:bad={testOnDesign}>
              {#each Array.from({ length: seriesShown ? seriesCount : 1 }) as _, i}
                <rect x={testX + (i % seriesCols) * 13} y={testY + Math.floor(i / seriesCols) * 13} width={TEST} height={TEST} rx="0.6" />
              {/each}
              <text x={testX} y={testY < 6 ? testY + TEST + 4.5 : testY - 2}>{t("Test")}</text>
            </g>
          {/if}
          {#if sim && simHead}
            <circle cx={simHead.x} cy={simHead.y} r="2.2" class="head" class:up={!simHead.down} />
          {/if}
        </svg>
        {#if step === 3}
          <p class="pick-hint"><Icon name="pointer" size={13} /> {t("In die Vorschau klicken: dort wird getestet.")}</p>
        {/if}
        <div class="simbar">
          <button class="btn outline" disabled={!plans.length || plotter.busy} onclick={toggleSim}>
            <Icon name={sim ? "stop" : "play"} /> {sim ? t("Simulation stoppen") : t("Simulation")}
          </button>
          {#if sim}
            <Segmented
              value={String(simSpeed)}
              onchange={(v) => (simSpeed = Number(v))}
              options={[
                { value: "1", label: "1×", hint: "Ungefähr in Echtzeit" },
                { value: "5", label: "5×" },
                { value: "20", label: "20×" },
              ]}
            />
            <div class="simprog"><div style:width="{(sim.pos / simTotal) * 100}%"></div></div>
          {/if}
        </div>
        <div class="legend">
          {#if groups.length > 1}
            {t("Reihenfolge:")}
            {#each groups as g, i}
              <span class="sw" style:background={g.color}></span> {i + 1}. {t(g.name)}{i < groups.length - 1 ? " →" : ""}
            {/each}
          {:else}
            {t("Reihenfolge:")} {t("zuerst")} <span class="sw" style:background={strokeColor(0, 2)}></span> {t("zuletzt")} <span class="sw" style:background={strokeColor(1, 2)}></span>
          {/if}
          <span class="dash"></span> {t("Leerfahrt")}
        </div>
      </div>

      <!-- Settings: a guided flow in four steps -->
      <div class="side">
        <nav class="steps" aria-label={t("Schritte")}>
          {#each STEPS as s}
            <button class="step" class:on={step === s.n} class:done={step > s.n} disabled={plotter.busy} onclick={() => go(s.n)} aria-current={step === s.n ? "step" : undefined}>
              <span class="num">{#if step > s.n}<Icon name="check" size={11} stroke={2.6} />{:else}{s.n}{/if}</span>
              <span class="step-label">{t(s.label)}</span>
            </button>
          {/each}
        </nav>

        <div class="step-body">
          {#if step === 1}
            <!-- 1 · Plotter and mat -->
            <div class="device" class:ok={ready} class:warn={plotter.status.state === "unloaded"} class:err={plotter.status.state === "error"}>
              <span class="dot"></span>
              <div class="grow-text">
                <div class="dev-name">{plotter.status.model ?? t("Kein Plotter")}</div>
                <div class="dev-state">{tMsg(plotter.status.message ?? STATE_TEXT[plotter.status.state])}</div>
              </div>
            </div>
            {#if !plotter.status.connected}
              <div class="callout">
                <p>{t("Plotter per USB anschließen und einschalten – er wird automatisch erkannt.")}</p>
                <button class="btn outline" onclick={() => { plotter.dialogOpen = false; ui.deviceMenuOpen = true; }}><Icon name="usb" /> {t("Per Bluetooth verbinden …")}</button>
              </div>
            {:else if plotter.status.state === "unloaded"}
              <div class="callout warn">
                <p>{mat === "none" ? t("Material einlegen und am Gerät die Lade-Taste drücken.") : t("Matte einlegen und am Gerät die Lade-Taste drücken.")}</p>
              </div>
            {/if}
            <div class="blade" class:worn={plotter.bladeWorn} title={t("Geschnittene Strecke seit dem letzten Messerwechsel")}>
              <Icon name="scissors" size={12} />
              {t("Messer: {len} seit {date}", { len: fmtLen(plotter.blade.mm), date: new Date(plotter.blade.since).toLocaleDateString(locale()) })}
              {#if plotter.bladeWorn}– {t("bitte prüfen")}{/if}
            </div>

            <section>
              <h3 class="section-title">{t("Worauf schneidest du?")}</h3>
              <div class="controls">
                <label class="select-row">
                  <span>{t("Matte")}</span>
                  <select class="field" bind:value={mat} disabled={plotter.busy}>
                    <option value="portrait8x12">{t(MAT_LABELS.portrait8x12)}</option>
                    <option value="portrait8_5x12">{t(MAT_LABELS.portrait8_5x12)}</option>
                    <option value="cameo12x12">{t(MAT_LABELS.cameo12x12)}</option>
                    <option value="cameo12x24">{t(MAT_LABELS.cameo12x24)}</option>
                    <option value="none">{t("Ohne Matte")}</option>
                  </select>
                </label>
                {#if mat === "none" && !reg}
                  <Toggle disabled={plotter.busy} label="Folie am Ende herausschieben" bind:checked={feedOut} hint="Schiebt das Material hinter das Motiv, damit es abgeschnitten werden kann" />
                {/if}
                <Toggle disabled={plotter.busy} label="Spiegeln (Flexfolie)" bind:checked={mirror} />
                {#if mirror}
                  <div class="mirror-mode">
                    <Segmented
                      value={mirrorMode}
                      onchange={setMirrorMode}
                      options={[
                        { value: "sheet", label: "Ganze Folie", hint: "Alles an der Mittelachse der Folie spiegeln – die Anordnung wird seitenverkehrt" },
                        { value: "each", label: "Jedes Element", hint: "Jedes Element an seiner eigenen Mittelachse spiegeln – alles bleibt an seinem Platz" },
                      ]}
                    />
                    <p class="hint">
                      {mirrorMode === "sheet"
                        ? t("Die ganze Folie wird seitenverkehrt – Elemente tauschen links und rechts.")
                        : t("Jedes Element wird an Ort und Stelle gespiegelt. Gruppen werden als Ganzes gespiegelt.")}
                    </p>
                  </div>
                {/if}
                <Toggle disabled={plotter.busy} label="Nur Auswahl schneiden" bind:checked={onlySelection} />
                <Toggle disabled={plotter.busy} label="Reihenfolge optimieren" bind:checked={optimize} hint="Innere Formen zuerst, kurze Leerfahrten" />
              </div>
              {#if matTooWide}
                <p class="warn-text">{t("Diese Matte ist breiter als der Plotter ({w} mm) – nur der Teil, der hineinpasst, wird geschnitten.", { w: fmt(plotter.status.widthMm ?? 0) })}</p>
              {/if}
              {#if reg}
                <p class="info-text">
                  <Icon name="sparkle" size={13} /> {t("Print & Cut: Bedruckten Bogen oben links auf die Matte legen. Die Passermarken werden vor dem Schnitt automatisch gesucht.")}
                </p>
              {/if}
            </section>
          {:else if step === 2}
            <!-- 2 · Material per cut colour -->
            <section>
              <h3 class="section-title">{colors.length > 1 ? t("Material je Schnittfarbe") : t("Material")}</h3>
              {#if colors.length > 1}
                <div class="color-tabs">
                  {#each order as c, i}
                    <div class="ctab-row">
                      <button class="ctab" class:on={c === active} onclick={() => selectColor(c)} disabled={plotter.busy} title={t("Einstellungen für diese Farbe")}>
                        <span class="sw" style:background={c}></span>
                        <span class="ctab-text">{i + 1}. {t(matOf(c).name)}</span>
                      </button>
                      <button class="btn icon mini" onclick={() => moveColor(c, -1)} disabled={plotter.busy || i === 0} title={t("Früher schneiden")} aria-label={t("Früher schneiden")}><Icon name="arrowUp" size={13} /></button>
                      <button class="btn icon mini" onclick={() => moveColor(c, 1)} disabled={plotter.busy || i === order.length - 1} title={t("Später schneiden")} aria-label={t("Später schneiden")}><Icon name="arrowDown" size={13} /></button>
                    </div>
                  {/each}
                </div>
                {#if misordered}
                  <p class="warn-text order-warn">
                    {t("Ein stärkerer Schnitt kommt vor einem leichteren – der Bogen kann sich lösen, bevor alles angeschnitten ist.")}
                    <button class="btn small-btn" onclick={() => (manualOrder = bySeverity(colors))} disabled={plotter.busy}>{t("Leichte zuerst")}</button>
                  </p>
                {/if}
              {/if}
              <div class="row">
                <select class="field" value={materialId} onchange={(e) => pickMaterial(e.currentTarget.value)} disabled={plotter.busy}>
                  <optgroup label={t("Vorlagen")}>
                    {#each PRESETS as p}<option value={p.id}>{t(p.name)}</option>{/each}
                  </optgroup>
                  {#if custom.length}
                    <optgroup label={t("Eigene")}>
                      {#each custom as p}<option value={p.id}>{p.name}</option>{/each}
                    </optgroup>
                  {/if}
                </select>
                <button class="btn icon outline" title={t("Als eigenes Material speichern")} onclick={() => (naming = modified ? `${t(m.name)} (${t("angepasst")})` : t(m.name))} disabled={plotter.busy}><Icon name="save" /></button>
                {#if materialId.startsWith("c-")}
                  <button class="btn icon outline danger" title={t("Material löschen")} onclick={deleteMaterial} disabled={plotter.busy}><Icon name="trash" /></button>
                {/if}
              </div>
              {#if naming !== null}
                <form class="row name-row" onsubmit={(e) => { e.preventDefault(); saveAsMaterial(); }}>
                  <!-- svelte-ignore a11y_autofocus -->
                  <input class="field" bind:value={naming} placeholder={t("Name des Materials")} autofocus />
                  <button class="btn primary" type="submit">{t("Speichern")}</button>
                  <button class="btn" type="button" onclick={() => (naming = null)}>{t("Abbrechen")}</button>
                </form>
              {/if}
              <!-- Summary of the values; the sliders only when wanted. -->
              <button class="values" class:open={tune || modified} onclick={() => (tune = !tune)} aria-expanded={tune || modified}>
                <span>
                  {m.tool === "pen" ? t("Stift") : `${t("Tiefe")} ${m.depth}`} · {t("Druck")} {m.pressure} · {t("Tempo")} {m.speed}{m.passes > 1 ? ` · ${m.passes}×` : ""}
                </span>
                <span class="values-link">{tune || modified ? t("Weniger") : t("Werte anpassen")}</span>
              </button>
              {#if tune || modified}
                <div class="controls">
                  {#if m.tool === "blade"}
                    <Slider disabled={plotter.busy} label="Messertiefe" bind:value={m.depth} min={1} max={10} hint="AutoBlade-Tiefe, wird vor dem Schnitt automatisch eingestellt" />
                  {/if}
                  <Slider disabled={plotter.busy} label="Druck" bind:value={m.pressure} min={1} max={33} />
                  <Slider disabled={plotter.busy} label="Geschwindigkeit" bind:value={m.speed} min={1} max={30} />
                  <Slider disabled={plotter.busy} label="Durchgänge" bind:value={m.passes} min={1} max={5} unit="×" />
                  {#if m.tool === "blade"}
                    <Slider disabled={plotter.busy} label="Überschnitt" bind:value={m.overcut} min={0} max={2} step={0.1} unit=" mm" hint="Schneidet geschlossene Formen etwas über den Startpunkt hinaus" />
                  {/if}
                  <label class="select-row">
                    <span>{t("Werkzeug")}</span>
                    <select class="field" bind:value={m.tool} disabled={plotter.busy}>
                      <option value="blade">{t("AutoBlade (Messer)")}</option>
                      <option value="pen">{t("Stift")}</option>
                    </select>
                  </label>
                </div>
              {/if}
              <p class="hint">{modified ? t("Angepasst – speichern, um die Werte zu behalten.") : t("Startwerte der Vorlage. Im nächsten Schritt kannst du sie an einer freien Stelle testen.")}</p>
            </section>
          {:else if step === 3}
            <!-- 3 · Test cut at a spot of your choice -->
            <section>
              <h3 class="section-title">{t("Testschnitt (empfohlen)")}</h3>
              <p class="lead">{t("Schneidet ein kleines Quadrat mit Dreieck. So siehst du vor dem eigentlichen Schnitt, ob Tiefe und Druck zum Material passen.")}</p>
              {#if colors.length > 1}
                <label class="select-row">
                  <span>{t("Testen mit")}</span>
                  <select class="field" value={active} onchange={(e) => selectColor(e.currentTarget.value)} disabled={plotter.busy}>
                    {#each order as c, i}<option value={c}>{i + 1}. {t(matOf(c).name)}</option>{/each}
                  </select>
                </label>
              {/if}
              <div class="test-pos">
                <span>{t("Stelle")}</span>
                <label>X <input class="field num" type="number" min="0" max={area.w - testW} step="0.5" bind:value={testX} /></label>
                <label>Y <input class="field num" type="number" min="0" max={area.h - testH} step="0.5" bind:value={testY} /></label>
                <span>mm</span>
              </div>
              {#if testOnDesign}
                <p class="warn-text">{t("Die Stelle liegt auf dem Motiv oder am Rand – in der Vorschau eine freie Stelle anklicken.")}</p>
              {:else}
                <p class="hint">{t("Am besten eine Ecke, die später nicht gebraucht wird.")}</p>
              {/if}

              {#if testFeedback && !plotter.busy}
                <div class="feedback">
                  <strong>{t("Wie sieht der Test aus?")}</strong>
                  <span class="hint">{m.tool === "pen" ? t("Linien gleichmäßig und gut sichtbar?") : t("Quadrat abziehen: Das Dreieck soll stehen bleiben, das Trägerpapier darf nur leicht angeritzt sein.")}</span>
                  <button class="btn primary" onclick={testGood}><Icon name="check" /> {t("Passt – weiter")}</button>
                  <div class="row">
                    <button class="btn outline grow" onclick={() => adjustTest(1)}>{m.tool === "pen" ? t("Zu schwach") : t("Nicht ganz durch")}</button>
                    <button class="btn outline grow" onclick={() => adjustTest(-1)}>{m.tool === "pen" ? t("Zu stark") : t("Zu tief")}</button>
                  </div>
                </div>
              {:else}
                <div class="row test-actions">
                  <button class="btn primary grow" disabled={!ready || testOnDesign} onclick={testCut}><Icon name="scissors" /> {t("Test schneiden")}</button>
                  <button class="btn outline" class:active={seriesOpen} disabled={plotter.busy} onclick={openSeries} title={t("Mehrere Testquadrate mit verschiedenen Kombinationen aus Messertiefe und Druck")}>{t("Testreihe")}</button>
                </div>
              {/if}
              {#if seriesOpen}
                <div class="series">
                  {#if seriesCut && !plotter.busy}
                    <span class="series-q">{t("Welches Quadrat ist am saubersten? So liegen sie auf der Matte:")}</span>
                    <table class="pick-grid">
                      <thead>
                        <tr>
                          <th>{isPen ? "" : t("Tiefe")} ↓ · {t("Druck")} →</th>
                          {#each seriesCut.pressures as p}<th>{p}</th>{/each}
                        </tr>
                      </thead>
                      <tbody>
                        {#each seriesCut.depths as d}
                          <tr>
                            <th>{isPen ? "" : d}</th>
                            {#each seriesCut.pressures as p}
                              <td>
                                <button
                                  class="cell"
                                  class:cur={p === m.pressure && (isPen || d === m.depth)}
                                  title={isPen ? `${t("Druck")} ${p}` : `${t("Messertiefe")} ${d} · ${t("Druck")} ${p}`}
                                  onclick={() => chooseSeries(d, p)}
                                  aria-label={isPen ? `${t("Druck")} ${p}` : `${t("Messertiefe")} ${d} · ${t("Druck")} ${p}`}
                                ></button>
                              </td>
                            {/each}
                          </tr>
                        {/each}
                      </tbody>
                    </table>
                    <button class="btn small-btn" onclick={() => (seriesCut = null)}>{t("Neue Testreihe")}</button>
                  {:else}
                    <span class="series-q">{t("Vorschläge für {m}", { m: t(m.name) })}</span>
                    <div class="presets">
                      {#each grids as g}
                        <button
                          class="preset"
                          class:on={same(g.depths, seriesDepths) && same(g.pressures, seriesPressures)}
                          title={t(g.hint)}
                          onclick={() => applyGrid(g)}
                        >
                          <b>{t(g.name)}</b>
                          <small>{isPen ? `${t("Druck")} ${g.pressures.join(" · ")}` : `${g.depths.length} × ${g.pressures.length}`}</small>
                        </button>
                      {/each}
                    </div>
                    {#if !isPen}
                      <label class="axis">
                        <span>{t("Messertiefe")}</span>
                        <input class="field" bind:value={depthText} placeholder="1, 2, 3" oninput={() => untrack(ensureFreeTestSpot)} />
                      </label>
                    {/if}
                    <label class="axis">
                      <span>{t("Druck")}</span>
                      <input class="field" bind:value={pressureText} placeholder="10, 14, 18" oninput={() => untrack(ensureFreeTestSpot)} />
                    </label>
                    <p class="hint">
                      {#if isPen}
                        {t("{n} Quadrate nebeneinander, Druck steigt nach rechts.", { n: seriesCount })}
                      {:else}
                        {t("{n} Quadrate: je Zeile eine Messertiefe (von oben), Druck steigt nach rechts.", { n: seriesCount })}
                      {/if}
                    </p>
                    {#if seriesCount > MAX_SQUARES}<p class="warn-text">{t("Höchstens {n} Quadrate auf einmal.", { n: MAX_SQUARES })}</p>{/if}
                    {#if seriesValid && !seriesFits}<p class="warn-text">{t("Die Reihe passt ab dieser Position nicht auf die Matte.")}</p>{/if}
                    <button class="btn outline" disabled={!ready || !seriesValid || !seriesFits || testOnDesign} onclick={cutSeries}><Icon name="scissors" /> {t("Testreihe schneiden")}</button>
                  {/if}
                </div>
              {/if}
              {#if !plotter.status.connected}
                <p class="hint">{t("Zum Testen den Plotter anschließen (Schritt 1).")}</p>
              {/if}
            </section>
            {#if reg}
              <section>
                <h3 class="section-title">{t("Print & Cut prüfen")}</h3>
                <div class="test">
                  <span>{t("Marken bei {x} / {y} mm, Abstand {w} × {h} mm", { x: reg.originX, y: reg.originY, w: fmt(reg.width), h: fmt(reg.length) })}</span>
                  <button class="btn outline" disabled={!ready} onclick={testMarks} title={t("Sucht nur die Passermarken, ohne zu schneiden")}>{t("Marken testen")}</button>
                </div>
                <div class="calib">
                  <div class="calib-head">
                    <span>{t("Feinjustierung Druck ↔ Schnitt")}</span>
                    <button class="btn small-btn" onclick={addCalibrationSheet} disabled={plotter.busy} title={t("Legt 4 gedruckte Quadrate mit Schnittrahmen auf die Seite")}>{t("Testbogen")}</button>
                  </div>
                  <div class="calib-row">
                    <label>X <input class="field num" type="number" step="0.1" bind:value={plotter.calib.dx} disabled={plotter.busy} onchange={() => plotter.saveCalib()} /> mm</label>
                    <label>Y <input class="field num" type="number" step="0.1" bind:value={plotter.calib.dy} disabled={plotter.busy} onchange={() => plotter.saveCalib()} /> mm</label>
                  </div>
                  <p class="hint">{t("Schnitt liegt zu weit rechts → X verringern; zu weit unten → Y verringern. Beim Testbogen ist der weiße Rand dann ringsum gleich (2 mm).")}</p>
                </div>
              </section>
            {/if}
          {:else}
            <!-- 4 · Cut -->
            <section>
              <h3 class="section-title">{t("Bereit zum Schneiden")}</h3>
              <ul class="passes">
                {#each groups as g, i (g.color)}
                  {@const mm = matOf(g.color)}
                  <li>
                    <span class="sw" style:background={g.color}></span>
                    <span class="pass-name">{groups.length > 1 ? `${i + 1}. ` : ""}{t(mm.name)}</span>
                    <span class="pass-vals">{mm.tool === "pen" ? t("Stift") : `${t("Tiefe")} ${mm.depth}`} · {t("Druck")} {mm.pressure}{mm.passes > 1 ? ` · ${mm.passes}×` : ""}</span>
                  </li>
                {/each}
              </ul>
              <div class="stats">
                {#if plans.length}
                  <span>{totals.paths === 1 ? t("1 Pfad") : t("{n} Pfade", { n: totals.paths })}</span>
                  <span><b>{fmtLen(totals.cut)}</b> {t("Schnitt")}</span>
                  <span><b>{fmtLen(totals.travel)}</b> {t("Leerweg")}</span>
                  <span title={t("Schätzung – wird mit jedem fertigen Schnitt genauer")}><b>≈ {fmtDuration(estimate)}</b> {t("Dauer")}</span>
                {:else if planError}
                  <span class="err-text">{planError}</span>
                {:else}
                  <span class="muted">{t("Keine Schnittlinien – erst tracen oder Formen zeichnen.")}</span>
                {/if}
              </div>
              {#if misordered}
                <p class="warn-text">{t("Ein stärkerer Schnitt kommt vor einem leichteren – der Bogen kann sich lösen, bevor alles angeschnitten ist.")}</p>
              {/if}
              {#if outside}
                <p class="warn-text">{reg ? t("Teile liegen außerhalb des Passermarken-Rahmens und werden abgeschnitten.") : t("Teile liegen außerhalb des Schneidebereichs und werden abgeschnitten.")}</p>
              {/if}
              {#if tooBig}
                <div class="tiles">
                  <div class="calib-head">
                    <span>{t("Großes Motiv: in Kacheln schneiden")}</span>
                    <input type="checkbox" bind:checked={tileOn} disabled={plotter.busy} title={t("Motiv auf mehrere Matten aufteilen")} />
                  </div>
                  {#if tiling}
                    <svg class="tilemap" viewBox="{tiling.box.x0 - 4} {tiling.box.y0 - 4} {tiling.box.x1 - tiling.box.x0 + 8} {tiling.box.y1 - tiling.box.y0 + 8}" preserveAspectRatio="xMidYMid meet">
                      {#each cutObjects as o (o.id)}
                        <path d={o.d} transform={mapTransform(o)} fill="none" stroke="var(--text-3)" stroke-width="1" vector-effect="non-scaling-stroke" />
                      {/each}
                      {#each tiling.tiles as tl, i}
                        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
                        <rect
                          x={tl.x}
                          y={tl.y}
                          width={tl.w}
                          height={tl.h}
                          class="tile"
                          class:on={i === tileIdx}
                          class:done={tilesDone.includes(i)}
                          vector-effect="non-scaling-stroke"
                          onclick={() => !plotter.busy && (tileIdx = i)}
                        />
                      {/each}
                    </svg>
                    <div class="tile-buttons">
                      {#each tiling.tiles as tl, i}
                        <button class="tbtn" class:on={i === tileIdx} class:done={tilesDone.includes(i)} disabled={plotter.busy} onclick={() => (tileIdx = i)} title={t("Reihe {r}, Spalte {c}", { r: tl.row + 1, c: tl.col + 1 })}>
                          {#if tilesDone.includes(i)}<Icon name="check" size={11} />{/if}{i + 1}
                        </button>
                      {/each}
                    </div>
                    <div class="calib-row">
                      <label>{t("Überlappung")} <input class="field num" type="number" min="0" max="30" step="1" bind:value={tileOverlap} disabled={plotter.busy} /> mm</label>
                      <span>{t("{n} Kacheln", { n: tiling.tiles.length })} · {fmt(cutArea.w)} × {fmt(cutArea.h)} mm</span>
                    </div>
                    <p class="hint">{mat === "none" ? t("Kachel {n} wird oben links auf dem Material geschnitten. Nach jedem Schnitt neues Material einlegen.", { n: tileIdx + 1 }) : t("Kachel {n} wird oben links auf der Matte geschnitten. Nach jedem Schnitt neues Material einlegen.", { n: tileIdx + 1 })}</p>
                  {:else}
                    <p class="hint">{t("Aus: alles außerhalb des Schneidebereichs wird abgeschnitten.")}</p>
                  {/if}
                </div>
              {/if}
              {#if resume}
                {@const p = resume.point}
                <div class="resume">
                  <div class="calib-head">
                    <span>{t("Unterbrochener Schnitt")}</span>
                    <button class="btn small-btn" onclick={() => (plotter.resume = null)} title={t("Nicht weiterschneiden")}>{t("Verwerfen")}</button>
                  </div>
                  <p class="hint">
                    {jobGroups.length > 1 ? `${t("Schritt {n}", { n: p.pass + 1 })} (${t(jobGroups[p.pass]?.name ?? "")}), ` : ""}{t("Pfad {a} von {b}.", { a: Math.min(p.stroke + 1, p.strokes), b: p.strokes })}
                    {t("Matte und Material nicht verschieben.")}
                  </p>
                  <div class="calib-row">
                    <span>{t("Weiter ab Pfad")}</span>
                    <button class="btn icon mini" onclick={() => (back = Math.min(p.stroke, back + 1))} disabled={resumeFrom === 0} aria-label={t("Einen Pfad früher")}>−</button>
                    <b>{resumeFrom + 1}</b>
                    <button class="btn icon mini" onclick={() => (back = Math.max(0, back - 1))} disabled={back === 0} aria-label={t("Einen Pfad später")}>+</button>
                  </div>
                  <button class="btn outline" disabled={!ready} onclick={() => startCut(true)}><Icon name="scissors" /> {t("Weiterschneiden")}</button>
                </div>
              {/if}
            </section>
          {/if}
        </div>

        <div class="footer">
          {#if plotter.busy}
            <div class="progress">
              <div class="bar" class:working={plotter.draining && plotter.phase === "cutting"}><div style:width="{pct}%"></div></div>
              <span>{progressText}</span>
            </div>
            <div class="actions">
              <button class="btn outline" disabled={plotter.phase === "cancelling"} onclick={() => plotter.pause(plotter.phase !== "paused")}>
                {plotter.phase === "paused" ? t("Fortsetzen") : t("Pause")}
              </button>
              <button class="btn outline danger grow" disabled={plotter.phase === "cancelling"} onclick={() => plotter.cancel()}><Icon name="x" /> {t("Abbrechen")}</button>
            </div>
          {:else if step === 4}
            <button class="btn primary big" disabled={!ready || totals.paths === 0} onclick={() => startCut()}>
              <Icon name="scissors" /> {allPen ? t("Zeichnen") : t("Schneiden")}{tiling ? ` – ${t("Kachel {a}/{b}", { a: tileIdx + 1, b: tiling.tiles.length })}` : ""}
            </button>
            {#if !plotter.status.connected}
              <p class="hint center">{t("Plotter per USB anschließen und einschalten.")}</p>
            {:else if plotter.status.state === "unloaded"}
              <p class="hint center">{t("Matte einlegen und am Gerät die Lade-Taste drücken.")}</p>
            {/if}
            <button class="btn link-btn" onclick={() => go(3)}><Icon name="chevronLeft" size={13} /> {t("Zurück")}</button>
          {:else}
            <div class="nav">
              {#if step > 1}
                <button class="btn outline" onclick={() => go((step - 1) as Step)}><Icon name="chevronLeft" /> {t("Zurück")}</button>
              {/if}
              <button class="btn primary grow" onclick={() => go((step + 1) as Step)}>{step === 3 ? t("Weiter zum Schneiden") : t("Weiter")} <Icon name="arrowRight" /></button>
            </div>
          {/if}
        </div>
      </div>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
    background: rgba(10, 10, 16, 0.45);
    backdrop-filter: blur(2px);
  }
  .dialog {
    display: flex;
    flex-direction: column;
    width: min(1120px, 100%);
    height: min(780px, 100%);
    border-radius: 14px;
    background: var(--panel);
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.35);
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    font-size: 15px;
  }
  .body {
    flex: 1;
    display: flex;
    min-height: 0;
  }
  .preview {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    padding: 18px;
    background: var(--canvas);
  }
  .preview svg {
    flex: 1;
    min-height: 0;
    width: 100%;
  }
  .mat {
    fill: var(--mat);
    stroke: var(--mat-border);
    stroke-width: 0.4;
  }
  .grid {
    stroke: var(--grid-major);
    stroke-width: 0.25;
  }
  .travel {
    stroke: var(--text-3);
    stroke-width: 0.3;
    stroke-dasharray: 1.2 1.2;
  }
  .origin {
    fill: var(--accent);
  }
  .head {
    fill: var(--accent);
    stroke: #fff;
    stroke-width: 0.5;
  }
  .head.up {
    fill: var(--panel);
    stroke: var(--accent);
  }
  .simbar {
    display: flex;
    align-items: center;
    gap: 10px;
    justify-content: center;
    padding-top: 8px;
  }
  .simprog {
    width: 160px;
    height: 6px;
    border-radius: 6px;
    background: var(--panel-2);
    border: 1px solid var(--border);
    overflow: hidden;
  }
  .simprog div {
    height: 100%;
    background: var(--accent);
  }
  .legend {
    display: flex;
    align-items: center;
    gap: 6px;
    justify-content: center;
    color: var(--text-3);
    font-size: 12px;
    padding-top: 10px;
  }
  .sw {
    width: 10px;
    height: 10px;
    border-radius: 3px;
    display: inline-block;
  }
  .dash {
    width: 18px;
    border-top: 1.5px dashed var(--text-3);
    margin-left: 10px;
  }
  .side {
    width: 340px;
    flex: none;
    display: flex;
    flex-direction: column;
    border-left: 1px solid var(--border);
    min-height: 0;
  }
  /* ---------- wizard ---------- */
  .steps {
    display: flex;
    gap: 2px;
    padding: 10px 10px 0;
    border-bottom: 1px solid var(--border);
    flex: none;
  }
  .step {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 4px 2px 9px;
    border: 0;
    border-bottom: 2px solid transparent;
    background: none;
    color: var(--text-3);
    font-size: 11.5px;
    cursor: pointer;
  }
  .step:disabled {
    cursor: default;
  }
  .step:hover:not(:disabled) {
    color: var(--text);
  }
  .step .num {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    border: 1.5px solid var(--border-strong);
    font-weight: 700;
    font-size: 11px;
  }
  .step.done {
    color: var(--text-2);
  }
  .step.done .num {
    border-color: var(--ok);
    background: var(--ok);
    color: #fff;
  }
  .step.on {
    color: var(--accent);
    border-bottom-color: var(--accent);
    font-weight: 600;
  }
  .step.on .num {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--accent-text);
  }
  .step-label {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .step-body {
    flex: 1;
    overflow-x: hidden;
    overflow-y: auto;
    min-height: 0;
  }
  .grow-text {
    flex: 1;
    min-width: 0;
  }
  .callout {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
    margin: 8px 16px 0;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--panel-2);
    color: var(--text-2);
    font-size: 12.5px;
  }
  .callout.warn {
    border-color: var(--warn);
    background: color-mix(in srgb, var(--warn) 8%, var(--panel));
  }
  .callout p {
    margin: 0;
  }
  .lead {
    margin: 0 0 12px;
    color: var(--text-2);
    font-size: 12.5px;
    line-height: 1.45;
  }
  .values {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    width: 100%;
    margin-top: 10px;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--panel-2);
    color: var(--text-2);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    text-align: left;
    cursor: pointer;
  }
  .values:hover {
    border-color: var(--border-strong);
  }
  .values-link {
    flex: none;
    color: var(--accent);
    font-weight: 600;
  }
  .test-pos {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 10px;
    color: var(--text-2);
    font-size: 12px;
  }
  .test-pos label {
    display: flex;
    align-items: center;
    gap: 5px;
  }
  .test-actions {
    margin-top: 12px;
  }
  .feedback {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 12px;
    padding: 12px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--accent);
    background: var(--accent-soft);
  }
  .feedback .hint {
    margin: 0;
  }
  .feedback .btn {
    justify-content: center;
  }
  .passes {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .passes li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--panel-2);
  }
  .pass-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 500;
  }
  .pass-vals {
    color: var(--text-3);
    font-size: 11.5px;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .nav {
    display: flex;
    gap: 8px;
  }
  .nav .grow,
  .feedback .grow,
  .test-actions .grow {
    flex: 1;
    justify-content: center;
  }
  .link-btn {
    align-self: center;
    height: 24px;
    color: var(--text-3);
    font-size: 12px;
  }
  .preview svg.picking {
    cursor: crosshair;
  }
  .testmark rect {
    fill: color-mix(in srgb, var(--accent) 18%, transparent);
    stroke: var(--accent);
    stroke-width: 0.5;
    stroke-dasharray: 1.2 0.8;
  }
  .testmark text {
    fill: var(--accent);
    font-size: 4px;
    font-weight: 700;
  }
  .testmark.bad rect {
    fill: color-mix(in srgb, var(--warn) 22%, transparent);
    stroke: var(--warn);
  }
  .testmark.bad text {
    fill: var(--warn);
  }
  .pick-hint {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    margin: 8px 0 0;
    color: var(--accent);
    font-size: 12px;
    font-weight: 600;
  }
  .device {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 14px 16px 4px;
    padding: 10px 12px;
    border-radius: var(--radius);
    background: var(--panel-2);
    border: 1px solid var(--border);
  }
  .device .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--text-3);
    flex: none;
  }
  .device.ok .dot {
    background: var(--ok);
    box-shadow: 0 0 0 4px color-mix(in srgb, var(--ok) 20%, transparent);
  }
  .device.warn .dot {
    background: var(--warn);
  }
  .device.err .dot {
    background: var(--danger);
  }
  .dev-name {
    font-weight: 600;
  }
  .dev-state {
    color: var(--text-2);
    font-size: 12px;
  }
  section {
    padding: 14px 16px;
    border-bottom: 1px solid var(--border);
  }
  .row {
    display: flex;
    gap: 6px;
  }
  .color-tabs {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-bottom: 10px;
  }
  .ctab-row {
    display: flex;
    gap: 4px;
  }
  .ctab-row .ctab {
    flex: 1;
    min-width: 0;
  }
  .mini {
    width: 26px;
    height: 30px;
    flex: none;
  }
  .order-warn {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    margin: 0 0 10px;
  }
  .ctab {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 30px;
    padding: 0 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--panel);
    cursor: pointer;
    text-align: left;
  }
  .ctab.on {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .ctab-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name-row {
    margin-top: 8px;
  }
  .controls {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin-top: 12px;
  }
  .hint {
    margin: 8px 0 0;
    color: var(--text-3);
    font-size: 12px;
  }
  .hint.center {
    text-align: center;
  }
  .select-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    color: var(--text-2);
  }
  .select-row select {
    width: 180px;
  }
  .stats {
    display: flex;
    gap: 14px;
    flex-wrap: wrap;
    margin-top: 14px;
    color: var(--text-2);
    font-size: 12px;
  }
  .stats b {
    color: var(--text);
    font-variant-numeric: tabular-nums;
  }
  .muted {
    color: var(--text-3);
  }
  .calib {
    margin-top: 12px;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: var(--panel-2);
    border: 1px solid var(--border);
  }
  .calib-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    color: var(--text-2);
    font-size: 12px;
    font-weight: 600;
  }
  .small-btn {
    height: 24px;
    font-size: 12px;
    border: 1px solid var(--border);
  }
  .calib-row {
    display: flex;
    gap: 14px;
    margin-top: 8px;
    color: var(--text-2);
    font-size: 12px;
  }
  .calib-row label {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .paper {
    fill: #fff;
  }
  .info-text {
    display: flex;
    gap: 6px;
    margin: 10px 0 0;
    color: var(--text-2);
    font-size: 12px;
    line-height: 1.45;
  }
  .info-text :global(svg) {
    flex: none;
    margin-top: 2px;
    color: var(--accent);
  }
  .mirror-mode {
    margin-top: -4px;
  }
  .mirror-mode .hint {
    margin-top: 6px;
  }
  .series {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: var(--panel-2);
    border: 1px solid var(--border);
  }
  .series .hint {
    margin: 0;
  }
  .presets {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
  }
  .preset {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
    padding: 6px 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--panel);
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .preset:hover {
    border-color: var(--border-strong);
  }
  .preset.on {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .preset b {
    font-size: 12px;
    font-weight: 600;
  }
  .preset small {
    color: var(--text-3);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }
  .axis {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-2);
    font-size: 12px;
  }
  .axis span {
    width: 80px;
    flex: none;
  }
  .axis input {
    flex: 1;
    min-width: 0;
  }
  .pick-grid {
    border-collapse: separate;
    border-spacing: 4px;
    font-size: 11.5px;
    font-variant-numeric: tabular-nums;
  }
  .pick-grid th {
    color: var(--text-3);
    font-weight: 600;
    text-align: center;
    white-space: nowrap;
  }
  .pick-grid thead th:first-child {
    text-align: left;
    font-weight: 500;
  }
  .cell {
    display: block;
    width: 28px;
    height: 28px;
    border: 1.5px dashed var(--border-strong);
    border-radius: 4px;
    background: var(--panel);
    cursor: pointer;
  }
  .cell:hover {
    border-style: solid;
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .cell.cur {
    border-style: solid;
    border-color: var(--text-3);
  }
  .series-q {
    color: var(--text-2);
    font-size: 12px;
  }
  .resume {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--warn);
    background: color-mix(in srgb, var(--warn) 8%, var(--panel));
  }
  .resume .hint {
    margin: 0;
  }
  .resume .calib-row {
    align-items: center;
    margin-top: 0;
  }
  .blade {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 5px;
    margin: 4px 16px 0;
    color: var(--text-3);
    font-size: 11.5px;
  }
  .blade.worn {
    color: var(--warn);
    font-weight: 600;
  }
  .tiles {
    margin-top: 12px;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: var(--panel-2);
    border: 1px solid var(--border);
  }
  .tiles input[type="checkbox"] {
    width: 16px;
    height: 16px;
    accent-color: var(--accent);
  }
  .tilemap {
    width: 100%;
    height: 150px;
    margin-top: 8px;
    background: var(--panel);
    border-radius: 6px;
  }
  .tile {
    fill: transparent;
    stroke: var(--border-strong);
    stroke-width: 1;
    cursor: pointer;
  }
  .tile:hover {
    fill: var(--accent-soft);
  }
  .tile.done {
    fill: color-mix(in srgb, var(--ok) 15%, transparent);
  }
  .tile.on {
    stroke: var(--accent);
    stroke-width: 2;
    fill: var(--accent-soft);
  }
  .tile-buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 8px;
  }
  .tbtn {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    min-width: 30px;
    height: 24px;
    padding: 0 6px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--panel);
    font-size: 12px;
    cursor: pointer;
  }
  .tbtn.done {
    color: var(--ok);
  }
  .tbtn.on {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
  }
  .warn-text {
    margin: 10px 0 0;
    color: var(--warn);
    font-size: 12px;
  }
  .err-text {
    color: var(--danger);
  }
  .footer {
    margin-top: auto;
    padding: 14px 16px 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    border-top: 1px solid var(--border);
    background: var(--panel);
    position: sticky;
    bottom: 0;
  }
  .test {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    color: var(--text-2);
    font-size: 12px;
  }
  .num {
    width: 56px;
    text-align: right;
  }
  .test .btn {
    margin-left: auto;
  }
  .big {
    height: 40px;
    justify-content: center;
    font-size: 14px;
  }
  .progress {
    display: flex;
    align-items: center;
    gap: 10px;
    font-variant-numeric: tabular-nums;
  }
  .bar {
    flex: 1;
    height: 8px;
    border-radius: 8px;
    background: var(--panel-2);
    border: 1px solid var(--border);
    overflow: hidden;
  }
  .bar div {
    height: 100%;
    background: linear-gradient(90deg, var(--accent), #ff7eb9);
    transition: width 0.3s;
  }
  /* Data is sent, the cutter is still busy: moving stripes instead of a
     frozen bar. */
  .bar.working div {
    background: repeating-linear-gradient(-45deg, var(--accent) 0 8px, #ff7eb9 8px 16px);
    background-size: 22.6px 22.6px;
    animation: stripes 0.8s linear infinite;
  }
  @keyframes stripes {
    to {
      background-position: 22.6px 0;
    }
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  .grow {
    flex: 1;
    justify-content: center;
  }
</style>
