<script lang="ts">
  import { api } from "../api";
  import { editor, regBracketPath } from "../editor.svelte";
  import { fmt, uid } from "../geometry";
  import { PRESETS, lastMaterialId, loadCustom, rememberMaterial, saveCustom, type Material } from "../materials";
  import { matKindFor, pathMatrix, plotter } from "../plotter.svelte";
  import type { CutPlan, CutSettings, MatKind, PathInput, PathObj, PathOptions } from "../types";
  import Icon from "./Icon.svelte";
  import Slider from "./Slider.svelte";
  import Toggle from "./Toggle.svelte";

  let custom = $state<Material[]>(loadCustom());
  const all = $derived([...PRESETS, ...custom]);
  const findMat = (id: string) => [...PRESETS, ...loadCustom()].find((p) => p.id === id);

  // Every cut colour is its own pass with its own material. `m` / `materialId`
  // are the working copy of the active colour; the others live in `byColor`.
  let byColor = $state<Record<string, { materialId: string; m: Material }>>({});
  let active = $state<string>("");
  let materialId = $state(lastMaterialId());
  let m = $state<Material>(structuredClone(findMat(lastMaterialId()) ?? PRESETS[0]));
  let mirror = $state(!!findMat(lastMaterialId())?.mirror);
  let mat = $state<MatKind>(matKindFor(editor.mat.name));
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
      m = structuredClone(entry.m);
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
    editor.toast(`Material „${name}" gespeichert`, "ok");
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

  /** Blue = through-cut, green = pen, otherwise the remembered material. */
  function defaultMaterialFor(color: string, index: number): string {
    if (color === "#3e63dd") return "sticker-through";
    if (color === "#30a46c") return "pen";
    if (color === "#e5484d") {
      // Red is the kiss-cut: never a through-cut or pen preset.
      const last = lastMaterialId();
      return ["sticker-through", "pen", "cardstock", "magnet"].includes(last) ? "sticker-matt" : last;
    }
    return index === 0 ? lastMaterialId() : "sticker-through";
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
  const order = $derived([...colors].sort((a, b) => severity(matOf(a)) - severity(matOf(b))));

  const groups = $derived(
    order.map((c) => {
      const mm = matOf(c);
      const objs = cutObjects.filter((o) => o.color.toLowerCase() === c);
      return {
        color: c,
        name: mm.name,
        paths: objs.map((o) => ({ d: o.d, matrix: pathMatrix(o, mirror ? editor.mat.w : null, editor.regmarks ? plotter.calib : undefined) })) as PathInput[],
        options: { overcut: mm.tool === "pen" ? 0 : mm.overcut, passes: mm.passes, optimize } as PathOptions,
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
        id: uid(), type: "path", name: "Kalibrierung", d: `M0 0H${s}V${s}H0Z`, bbox: [0, 0, s, s], color: "#e5484d", fill: false,
        x: cx - s / 2, y: cy - s / 2, w: s, h: s, rot: 0, flipX: false, flipY: false, visible: true, locked: false, group: g, ...extra,
      });
      return [sq(10, { printFill: "#000000", cut: false, color: "#8b919c" }), sq(14, {})];
    });
    editor.add(objs);
    editor.toast("Testbogen angelegt: drucken, schneiden, dann den weißen Rand um die Quadrate prüfen.", "ok");
  }
  const area = $derived(reg ? { w: editor.mat.w, h: editor.mat.h } : mat === "portrait8_5x12" ? { w: 215.9, h: 304.8 } : mat === "portrait8x12" ? { w: 203.2, h: 304.8 } : { w: plotter.status.widthMm ?? 203, h: editor.mat.h });
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

  async function startCut() {
    try {
      plotter.begin();
      const r = await api.cutStart(
        $state.snapshot(groups).map((g) => ({ paths: g.paths, options: g.options, settings: g.settings })),
        reg ? { ...reg } : null,
      );
      if (r.clamped > 0) editor.toast(`${r.clamped} Punkte lagen außerhalb und wurden an den Rand gesetzt.`, "error");
    } catch (e) {
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
      const st = settings ?? {
        tool: m.tool, pressure: m.pressure, speed: m.speed, depth: m.tool === "blade" ? m.depth : null, mat, bladeOffset: 0.9, liftCorners: false,
      };
      await api.cutTest(testX, testY, $state.snapshot(st) as CutSettings);
    } catch (e) {
      plotter.phase = "idle";
      editor.error(e);
    }
  }

  function close() {
    if (!plotter.busy) plotter.dialogOpen = false;
  }

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

  const fmtLen = (mm: number) => (mm >= 1000 ? `${fmt(mm / 1000, 2)} m` : `${fmt(mm / 10, 1)} cm`);
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

<div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && close()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label="Schneiden">
    <header>
      <h2><Icon name="scissors" size={18} /> Schneiden</h2>
      <button class="btn icon" onclick={close} disabled={plotter.busy} aria-label="Schließen"><Icon name="x" /></button>
    </header>

    <div class="body">
      <!-- Preview -->
      <div class="preview">
        <svg viewBox="-6 -6 {area.w + 12} {area.h + 12}" preserveAspectRatio="xMidYMid meet">
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
              <line x1={prev[0]} y1={prev[1]} x2={st[0][0]} y2={st[0][1]} class="travel" />
              <polyline
                points={st.map((q) => q.join(",")).join(" ")}
                fill="none"
                stroke={multi ? groups[gi]?.color : strokeColor(i, plan.strokes.length)}
                stroke-width="0.5"
                stroke-linejoin="round"
              />
            {/each}
          {/each}
          {#if plans.length}
            <circle cx={reg ? reg.originX : 0} cy={reg ? reg.originY : 0} r="1.5" class="origin" />
          {/if}
        </svg>
        <div class="legend">
          {#if groups.length > 1}
            Reihenfolge:
            {#each groups as g, i}
              <span class="sw" style:background={g.color}></span> {i + 1}. {g.name}{i < groups.length - 1 ? " →" : ""}
            {/each}
          {:else}
            Reihenfolge: zuerst <span class="sw" style:background={strokeColor(0, 2)}></span> zuletzt <span class="sw" style:background={strokeColor(1, 2)}></span>
          {/if}
          <span class="dash"></span> Leerfahrt
        </div>
      </div>

      <!-- Settings -->
      <div class="side">
        <div class="device" class:ok={ready} class:warn={plotter.status.state === "unloaded"} class:err={plotter.status.state === "error"}>
          <span class="dot"></span>
          <div>
            <div class="dev-name">{plotter.status.model ?? "Kein Plotter"}</div>
            <div class="dev-state">{plotter.status.message ?? STATE_TEXT[plotter.status.state]}</div>
          </div>
        </div>

        <section>
          <h3 class="section-title">Material{colors.length > 1 ? " je Schnittfarbe" : ""}</h3>
          {#if colors.length > 1}
            <div class="color-tabs">
              {#each order as c, i}
                <button class="ctab" class:on={c === active} onclick={() => selectColor(c)} disabled={plotter.busy} title="Einstellungen für diese Farbe">
                  <span class="sw" style:background={c}></span>
                  <span class="ctab-text">{i + 1}. {matOf(c).name}</span>
                </button>
              {/each}
            </div>
          {/if}
          <div class="row">
            <select class="field" value={materialId} onchange={(e) => pickMaterial(e.currentTarget.value)} disabled={plotter.busy}>
              <optgroup label="Vorlagen">
                {#each PRESETS as p}<option value={p.id}>{p.name}</option>{/each}
              </optgroup>
              {#if custom.length}
                <optgroup label="Eigene">
                  {#each custom as p}<option value={p.id}>{p.name}</option>{/each}
                </optgroup>
              {/if}
            </select>
            <button class="btn icon outline" title="Als eigenes Material speichern" onclick={() => (naming = modified ? `${m.name} (angepasst)` : m.name)} disabled={plotter.busy}><Icon name="save" /></button>
            {#if materialId.startsWith("c-")}
              <button class="btn icon outline danger" title="Material löschen" onclick={deleteMaterial} disabled={plotter.busy}><Icon name="trash" /></button>
            {/if}
          </div>
          {#if naming !== null}
            <form class="row name-row" onsubmit={(e) => { e.preventDefault(); saveAsMaterial(); }}>
              <!-- svelte-ignore a11y_autofocus -->
              <input class="field" bind:value={naming} placeholder="Name des Materials" autofocus />
              <button class="btn primary" type="submit">Speichern</button>
              <button class="btn" type="button" onclick={() => (naming = null)}>Abbrechen</button>
            </form>
          {/if}
          <p class="hint">{modified ? "Angepasst – speichern, um die Werte zu behalten." : "Startwerte – vorher mit einem Testschnitt prüfen."}</p>
          <div class="controls">
            {#if m.tool === "blade"}
              <Slider label="Messertiefe" bind:value={m.depth} min={1} max={10} hint="AutoBlade-Tiefe, wird vor dem Schnitt automatisch eingestellt" />
            {/if}
            <Slider label="Druck" bind:value={m.pressure} min={1} max={33} />
            <Slider label="Geschwindigkeit" bind:value={m.speed} min={1} max={30} />
            <Slider label="Durchgänge" bind:value={m.passes} min={1} max={5} unit="×" />
            {#if m.tool === "blade"}
              <Slider label="Überschnitt" bind:value={m.overcut} min={0} max={2} step={0.1} unit=" mm" hint="Schneidet geschlossene Formen etwas über den Startpunkt hinaus" />
            {/if}
            <label class="select-row">
              <span>Werkzeug</span>
              <select class="field" bind:value={m.tool}>
                <option value="blade">AutoBlade (Messer)</option>
                <option value="pen">Stift</option>
              </select>
            </label>
          </div>
        </section>

        <section>
          <h3 class="section-title">Auftrag</h3>
          <div class="controls">
            <label class="select-row">
              <span>Matte</span>
              <select class="field" bind:value={mat}>
                <option value="portrait8x12">Matte 8″ × 12″</option>
                <option value="portrait8_5x12">Matte 8,5″ × 12″</option>
                <option value="none">Ohne Matte</option>
              </select>
            </label>
            <Toggle label="Spiegeln (Flexfolie)" bind:checked={mirror} />
            <Toggle label="Nur Auswahl schneiden" bind:checked={onlySelection} />
            <Toggle label="Reihenfolge optimieren" bind:checked={optimize} hint="Innere Formen zuerst, kurze Leerfahrten" />
          </div>
          <div class="stats">
            {#if plans.length}
              <span><b>{totals.paths}</b> Pfade</span>
              <span><b>{fmtLen(totals.cut)}</b> Schnitt</span>
              <span><b>{fmtLen(totals.travel)}</b> Leerweg</span>
              {#if plans.length > 1}<span><b>{plans.length}</b> Durchgänge</span>{/if}
            {:else if planError}
              <span class="err-text">{planError}</span>
            {:else}
              <span class="muted">Keine Schnittlinien – erst tracen oder Formen zeichnen.</span>
            {/if}
          </div>
          {#if outside}
            <p class="warn-text">Teile liegen außerhalb {reg ? "des Passermarken-Rahmens" : "des Schneidebereichs"} und werden abgeschnitten.</p>
          {/if}
          {#if reg}
            <div class="calib">
              <div class="calib-head">
                <span>Feinjustierung Druck ↔ Schnitt</span>
                <button class="btn small-btn" onclick={addCalibrationSheet} title="Legt 4 gedruckte Quadrate mit Schnittrahmen auf die Seite">Testbogen</button>
              </div>
              <div class="calib-row">
                <label>X <input class="field num" type="number" step="0.1" bind:value={plotter.calib.dx} onchange={() => plotter.saveCalib()} /> mm</label>
                <label>Y <input class="field num" type="number" step="0.1" bind:value={plotter.calib.dy} onchange={() => plotter.saveCalib()} /> mm</label>
              </div>
              <p class="hint">Schnitt liegt zu weit rechts → X verringern; zu weit unten → Y verringern. Beim Testbogen ist der weiße Rand dann ringsum gleich (2 mm).</p>
            </div>
            <p class="info-text">
              <Icon name="sparkle" size={13} /> Print & Cut: Bedruckten Bogen oben links auf die Matte legen. Die Passermarken werden vor dem Schnitt automatisch gesucht.
            </p>
          {/if}
        </section>

        <div class="footer">
          {#if plotter.busy}
            <div class="progress">
              <div class="bar"><div style:width="{pct}%"></div></div>
              <span>{plotter.registering ? "Suche Passermarken …" : plotter.phase === "paused" ? "Pausiert" : plotter.phase === "cancelling" ? "Breche ab …" : pct >= 100 ? "Schneidet die letzten Linien …" : groups.length > 1 ? `Durchgang ${plotter.pass + 1}/${groups.length} · ${pct} %` : `${pct} %`}</span>
            </div>
            <div class="actions">
              <button class="btn outline" disabled={plotter.phase === "cancelling"} onclick={() => plotter.pause(plotter.phase !== "paused")}>
                {plotter.phase === "paused" ? "Fortsetzen" : "Pause"}
              </button>
              <button class="btn outline danger grow" disabled={plotter.phase === "cancelling"} onclick={() => plotter.cancel()}><Icon name="x" /> Abbrechen</button>
            </div>
          {:else}
            {#if reg}
              <div class="test">
                <span>Marken bei {reg.originX} / {reg.originY} mm, Abstand {fmt(reg.width)} × {fmt(reg.length)} mm</span>
                <button class="btn outline" disabled={!ready} onclick={testMarks} title="Sucht nur die Passermarken, ohne zu schneiden">Marken testen</button>
              </div>
            {/if}
            <div class="test">
              <span>Testschnitt bei</span>
              <input class="field num" type="number" min="0" max={area.w - 10} bind:value={testX} />
              <span>×</span>
              <input class="field num" type="number" min="0" max={area.h - 10} bind:value={testY} />
              <span>mm</span>
              <button class="btn outline" disabled={!ready} onclick={testCut}>Test</button>
            </div>
            <button class="btn primary big" disabled={!ready || totals.paths === 0} onclick={startCut}>
              <Icon name="scissors" /> {m.tool === "pen" ? "Zeichnen" : "Schneiden"}
            </button>
            {#if !plotter.status.connected}
              <p class="hint center">Plotter per USB anschließen und einschalten.</p>
            {:else if plotter.status.state === "unloaded"}
              <p class="hint center">Matte einlegen und am Gerät die Lade-Taste drücken.</p>
            {/if}
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
    overflow-y: auto;
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
    background: linear-gradient(90deg, var(--accent), #c04bff);
    transition: width 0.3s;
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
