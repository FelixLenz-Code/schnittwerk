<script lang="ts">
  import { untrack } from "svelte";
  import { api } from "../api";
  import { DEFAULT_TRACE, editor } from "../editor.svelte";
  import { center, rotate, uid } from "../geometry";
  import type { ImageObj, PathObj, TraceMode } from "../types";
  import Icon from "./Icon.svelte";
  import { t } from "../i18n.svelte";
  import Segmented from "./Segmented.svelte";
  import Slider from "./Slider.svelte";
  import Toggle from "./Toggle.svelte";
  import { brush } from "../brush.svelte";
  import { PAPERS, perspective } from "../perspective.svelte";
  import Section from "./Section.svelte";
  import { ui } from "../ui.svelte";

  type Kind = TraceMode["kind"];

  // Per-mode settings survive switching between modes.
  let threshold = $state({ auto: true, level: 128, invert: false });
  let colorKey = $state({ color: [0, 0, 0] as [number, number, number], tolerance: 18 });
  let multi = $state({ colors: 4, skipBackground: true });
  let hideImage = $state(true);

  const target = $derived.by((): ImageObj | null => {
    const s = editor.selected;
    if (s.length === 1 && s[0].type === "image") return s[0];
    return null;
  });
  const asset = $derived(target ? editor.images[target.imageId] : null);
  const kind = $derived(editor.traceParams.mode.kind);
  const p = editor.traceParams;

  function setKind(k: Kind) {
    editor.traceParams.mode = buildMode(k);
  }

  function buildMode(k: Kind): TraceMode {
    switch (k) {
      case "threshold":
      case "centerline":
        return { kind: k, level: threshold.auto ? null : threshold.level, invert: threshold.invert };
      case "colorKey":
        return { kind: k, color: [...colorKey.color], tolerance: colorKey.tolerance };
      case "multiColor":
        return { kind: k, colors: multi.colors, skipBackground: multi.skipBackground };
      case "alpha":
        return { kind: k };
    }
  }

  // Keep the mode object in sync with the per-mode controls.
  $effect(() => {
    const m = buildMode(kind);
    if (JSON.stringify(m) !== JSON.stringify(editor.traceParams.mode)) editor.traceParams.mode = m;
  });

  // ---------- live tracing ----------
  let seq = 0;
  $effect(() => {
    const key = JSON.stringify(editor.traceParams);
    const obj = target;
    if (!obj || !asset) {
      editor.tracePreview = null;
      return;
    }
    const my = ++seq;
    const timer = setTimeout(async () => {
      editor.tracing = true;
      const t0 = performance.now();
      try {
        const result = await api.traceImage(obj.imageId, JSON.parse(key));
        if (my === seq) editor.tracePreview = { objId: obj.id, result, ms: Math.round(performance.now() - t0) };
      } catch (e) {
        if (my === seq) editor.error(e);
      } finally {
        if (my === seq) editor.tracing = false;
      }
    }, 160);
    return () => clearTimeout(timer);
  });

  $effect(() => () => {
    // Leaving the panel removes the preview and ends brush mode.
    seq++;
    editor.tracePreview = null;
    editor.picking = null;
    brush.mode = null;
    perspective.cancel();
  });

  // Selecting another object ends the perspective tool.
  $effect(() => {
    if (perspective.objId && target?.id !== perspective.objId) perspective.cancel();
  });

  const preview = $derived(editor.tracePreview && target && editor.tracePreview.objId === target.id ? editor.tracePreview : null);
  const contours = $derived(preview?.result.layers.reduce((n, l) => n + l.contours, 0) ?? 0);

  function pickColor() {
    if (!target) return;
    const id = target.imageId;
    editor.picking = async (x, y) => {
      try {
        colorKey.color = await api.sampleColor(id, x, y);
      } catch (e) {
        editor.error(e);
      }
    };
  }

  // With "Auto" the slider shows the threshold the image analysis chose.
  $effect(() => {
    const auto = preview?.result.threshold;
    if (threshold.auto && auto != null) untrack(() => (threshold.level = auto));
  });

  const hex = $derived("#" + colorKey.color.map((c) => c.toString(16).padStart(2, "0")).join(""));

  function apply() {
    if (!preview || !target) return;
    const img = target;
    const r = preview.result;
    const sx = img.w / r.width, sy = img.h / r.height;
    const ic = center(img);
    const objs: PathObj[] = r.layers.map((l, i) => {
      const [bx, by, bw, bh] = l.bbox;
      const w = bw * sx, h = bh * sy;
      // Center of the layer in the image's unrotated box, relative to its center.
      let lx = (bx + bw / 2) * sx - img.w / 2;
      let ly = (by + bh / 2) * sy - img.h / 2;
      if (img.flipX) lx = -lx;
      if (img.flipY) ly = -ly;
      const off = rotate({ x: lx, y: ly }, img.rot);
      return {
        id: uid(),
        type: "path",
        name: r.layers.length > 1 ? `${img.name} · ${t("Farbe {n}", { n: i + 1 })}` : kind === "centerline" ? `${img.name} · ${t("Linien")}` : `${img.name} · ${t("Kontur")}`,
        d: l.d,
        bbox: l.bbox,
        color: kind === "multiColor" ? l.color : kind === "centerline" ? "#30a46c" : "#e5484d",
        fill: kind === "multiColor",
        x: ic.x + off.x - w / 2,
        y: ic.y + off.y - h / 2,
        w,
        h,
        rot: img.rot,
        flipX: img.flipX,
        flipY: img.flipY,
        visible: true,
        locked: false,
      };
    });
    editor.add(objs);
    if (hideImage) {
      const o = editor.byId(img.id);
      if (o) o.visible = false;
    }
    editor.toast(t("{n} Konturen übernommen", { n: contours }), "ok");
    editor.rightTab = "object";
    ui.nextSteps = { kind: "trace", sel: editor.selection.join(",") };
  }

  function reset() {
    Object.assign(editor.traceParams, structuredClone({ ...DEFAULT_TRACE, mode: editor.traceParams.mode }));
    threshold = { auto: true, level: 128, invert: false };
  }
</script>

{#if !target || !asset}
  <div class="empty">
    <div class="empty-icon"><Icon name="wand" size={28} stroke={1.5} /></div>
    <p><strong>{t("Bild auswählen zum Tracen")}</strong></p>
    <p class="muted">{t("Importiere ein Foto, eine Zeichnung oder ein Logo und wähle es auf der Arbeitsfläche aus.")}</p>
  </div>
{:else}
  <div class="panel">
    <div class="head">
      <img src={asset.src} alt="" />
      <div>
        <div class="name">{asset.name}</div>
        <div class="muted">{asset.pxW} × {asset.pxH} px</div>
      </div>
    </div>

    <section>
      <h3 class="section-title">{t("Modus")}</h3>
      <Segmented
        value={kind}
        onchange={setKind}
        options={[
          { value: "threshold", label: "S/W", hint: "Dunkle Bereiche werden zu Formen – ideal für Zeichnungen, Schrift und Logos" },
          { value: "colorKey", label: "Farbe", hint: "Alles in einer gewählten Farbe tracen" },
          { value: "multiColor", label: "Mehrfarbig", hint: "Bild in mehrere Farbebenen zerlegen" },
          { value: "alpha", label: "Freistellen", hint: "Umriss des nicht-transparenten Bereichs (PNG mit Transparenz)" },
          { value: "centerline", label: "Linie", hint: "Eine Linie mitten durch jeden Strich – zum Zeichnen mit dem Stift oder für filigrane Linien" },
        ]}
        cols={3}
      />

      <div class="controls">
        {#if kind === "threshold" || kind === "centerline"}
          <div class="with-auto">
            <Slider
              label="Schwellwert"
              bind:value={threshold.level}
              min={1}
              max={254}
              oninput={() => (threshold.auto = false)}
              display={threshold.auto && preview?.result.threshold == null ? "–" : String(threshold.level)}
              hint="Pixel dunkler als dieser Wert werden zur Form – Ziehen stellt von Hand ein"
            />
            <button
              class="auto"
              class:on={threshold.auto}
              title={t("Schwellwert automatisch aus dem Bild bestimmen")}
              onclick={() => {
                if (!threshold.auto && preview) threshold.level = preview.result.threshold ?? threshold.level;
                threshold.auto = !threshold.auto;
              }}>Auto</button
            >
          </div>
          <Toggle label="Invertieren (helle Bereiche)" bind:checked={threshold.invert} />
          {#if kind === "centerline"}
            <p class="muted">{t("Ergebnis sind offene Linien (grün = mit dem Stift zeichnen). „Flecken entfernen“ verwirft kurze Linien.")}</p>
          {/if}
        {:else if kind === "colorKey"}
          <div class="color-row">
            <span class="swatch" style:background={hex}></span>
            <input class="field mono" value={hex} readonly />
            <button class="btn outline" class:active={!!editor.picking} onclick={pickColor} title={t("Farbe im Bild aufnehmen")}>
              <Icon name="pipette" /> {t("Pipette")}
            </button>
          </div>
          <Slider label="Toleranz" bind:value={colorKey.tolerance} min={1} max={60} unit=" %" />
        {:else if kind === "multiColor"}
          <Slider label="Anzahl Farben" bind:value={multi.colors} min={2} max={12} />
          <Toggle label="Hintergrund weglassen" bind:checked={multi.skipBackground} hint="Die Farbe am Bildrand wird nicht getraced" />
          {#if preview}
            <div class="palette">
              {#each preview.result.layers as l}
                <span class="swatch" style:background={l.color} title={l.color}></span>
              {/each}
            </div>
          {/if}
        {:else}
          <p class="muted">{t("Zeichnet den Umriss aller nicht transparenten Pixel – perfekt für freigestellte Sticker-Motive.")}</p>
        {/if}
      </div>
    </section>

    <Section id="trace-prepare" title={t("Bild vorbereiten")} open={false}>
      <h4 class="sub">{t("Foto einer Zeichnung")}</h4>
      {#if perspective.objId && !perspective.custom}
        <p class="muted">
          {perspective.detected ? t("Blatt erkannt – Ecken bei Bedarf nachziehen.") : t("Kein Blatt erkannt – die vier Punkte auf die Ecken ziehen.")}
        </p>
        <div class="controls">
          <label class="select-row">
            <span>{t("Format")}</span>
            <select class="field" bind:value={perspective.paper} title={t("Mit Papierformat wird das Bild in Originalgröße angelegt")}>
              {#each Object.entries(PAPERS) as [id, p]}<option value={id}>{t(p.label)}</option>{/each}
            </select>
          </label>
        </div>
        <div class="brush-row persp-actions">
          <button class="btn outline" onclick={() => perspective.cancel()} disabled={perspective.busy}>{t("Abbrechen")}</button>
          <button class="btn primary grow" onclick={() => perspective.apply()} disabled={perspective.busy}>
            <Icon name="check" /> {t("Entzerren")}
          </button>
        </div>
      {:else}
        <div class="brush-row">
          <button class="btn outline" onclick={() => target && perspective.start(target)} disabled={perspective.busy} title={t("Schräg fotografiertes Blatt gerade ziehen")}>
            {t("Perspektive korrigieren")}
          </button>
        </div>
      {/if}
      <div class="controls">
        <Toggle label="Beleuchtung ausgleichen" bind:checked={p.flatten} hint="Schatten und Helligkeitsverläufe auf dem Papier entfernen – vor allem bei Handyfotos" />
      </div>
      <h4 class="sub">{t("Bild bereinigen")}</h4>
      <div class="brush-row">
        <button class="btn outline" class:active={brush.mode === "erase"} onclick={() => brush.toggle("erase")} title={t("Flecken und Störungen wegradieren (wird transparent)")}>{t("Radierer")}</button>
        <button class="btn outline" class:active={brush.mode === "paint"} onclick={() => brush.toggle("paint")} title={t("Lücken schließen, Linien ergänzen")}>{t("Pinsel")}</button>
        {#if brush.mode === "paint"}
          <input type="color" bind:value={brush.color} title={t("Pinselfarbe")} />
        {/if}
      </div>
      {#if brush.mode}
        <div class="controls">
          <Slider label="Größe" bind:value={brush.size} min={0.5} max={20} step={0.5} unit=" mm" />
          <p class="muted">{t("Auf dem Bild malen – jeder Strich wird sofort übernommen und neu getraced. Rückgängig mit")} <kbd>{t("Strg")}</kbd>+<kbd>Z</kbd>.</p>
        </div>
      {/if}
      <h4 class="sub">{t("Helligkeit & Kontrast")}</h4>
      <div class="controls">
        <Slider label="Helligkeit" bind:value={p.brightness} min={-100} max={100} />
        <Slider label="Kontrast" bind:value={p.contrast} min={-100} max={100} />
        <Slider label="Weichzeichnen" bind:value={p.blur} min={0} max={6} step={0.25} unit=" px" hint="Glättet Rauschen und Papierstruktur vor dem Tracen" />
      </div>
    </Section>

    <Section id="trace-contour" title={t("Kontur")}>
      <div class="controls first">
        <Slider label="Glättung" bind:value={p.smoothness} min={0} max={100} hint="Niedrig = folgt jeder Ecke, hoch = runde, ruhige Kurven" />
        <Slider label="Flecken entfernen" bind:value={p.despeckle} min={0} max={2000} step={4} unit=" px²" hint="Kleinere Formen werden verworfen" />
        <Slider label="Dicker / dünner" bind:value={p.grow} min={-10} max={10} unit=" px" hint="Formen vor dem Tracen wachsen (+) oder schrumpfen (–) lassen" />
        <Toggle label="Nur Außenkontur" bind:checked={p.outerOnly} hint="Löcher füllen – ergibt eine geschlossene Silhouette" />
        <Toggle label="Kurven (statt Ecken)" bind:checked={p.curves} />
        <label class="select-row">
          <span>{t("Auflösung")}</span>
          <select class="field" bind:value={p.maxSize}>
            <option value={1000}>{t("Schnell (1000 px)")}</option>
            <option value={2000}>{t("Normal (2000 px)")}</option>
            <option value={4000}>{t("Fein (4000 px)")}</option>
            <option value={0}>{t("Original")}</option>
          </select>
        </label>
      </div>
    </Section>

    <div class="footer">
      <div class="stats">
        {#if editor.tracing}
          <span class="dot busy"></span> {t("Berechne …")}
        {:else if preview}
          <span class="dot"></span>
          {preview.result.layers.length === 1 ? t("1 Ebene") : t("{n} Ebenen", { n: preview.result.layers.length })} · {t("{n} Konturen", { n: contours })} · {preview.ms} ms
        {:else}
          –
        {/if}
      </div>
      <Toggle label="Bild danach ausblenden" bind:checked={hideImage} />
      <div class="actions">
        <button class="btn outline" onclick={reset}>{t("Zurücksetzen")}</button>
        <button class="btn primary grow" disabled={!preview || contours === 0} onclick={apply}>
          <Icon name="check" /> {t("Übernehmen")}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .persp-actions {
    margin-top: 10px;
  }
  .persp-actions .grow {
    flex: 1;
    justify-content: center;
  }
  .panel {
    display: flex;
    flex-direction: column;
    min-height: 100%;
  }
  .head {
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 14px 16px;
    border-bottom: 1px solid var(--border);
  }
  .head img {
    width: 44px;
    height: 44px;
    object-fit: contain;
    border-radius: 6px;
    background: repeating-conic-gradient(var(--panel-2) 0 25%, var(--border) 0 50%) 0 0 / 10px 10px;
    border: 1px solid var(--border);
  }
  .name {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 200px;
  }
  section {
    padding: 14px 16px;
    border-bottom: 1px solid var(--border);
  }
  .controls {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin-top: 12px;
  }
  section > .controls:first-child,
  .controls.first {
    margin-top: 0;
  }
  .sub {
    margin: 14px 0 8px;
    color: var(--text-2);
    font-size: 12px;
    font-weight: 600;
  }
  .sub:first-child {
    margin-top: 2px;
  }
  .with-auto {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }
  .with-auto > :global(:first-child) {
    flex: 1;
    min-width: 0;
  }
  .auto {
    height: 22px;
    padding: 0 8px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--panel);
    color: var(--text-2);
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
  }
  .auto.on {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
  }
  .muted {
    color: var(--text-3);
    margin: 0;
  }
  .color-row {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .mono {
    font-family: ui-monospace, monospace;
  }
  .swatch {
    width: 22px;
    height: 22px;
    flex: none;
    border-radius: 6px;
    border: 1px solid var(--border-strong);
  }
  .palette {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .brush-row {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .brush-row input[type="color"] {
    width: 30px;
    height: 28px;
    padding: 0;
    border: 0;
    background: none;
  }
  kbd {
    font: inherit;
    font-size: 11px;
    padding: 0 4px;
    border: 1px solid var(--border);
    border-radius: 4px;
  }
  .select-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    color: var(--text-2);
  }
  .select-row select {
    width: 160px;
  }
  .footer {
    position: sticky;
    bottom: 0;
    margin-top: auto;
    padding: 12px 16px 14px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    background: var(--panel);
    border-top: 1px solid var(--border);
  }
  .stats {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-2);
    font-variant-numeric: tabular-nums;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--ok);
  }
  .dot.busy {
    background: var(--warn);
    animation: pulse 0.8s infinite alternate;
  }
  @keyframes pulse {
    to {
      opacity: 0.3;
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
  .empty {
    padding: 48px 28px;
    text-align: center;
    color: var(--text-2);
  }
  .empty p {
    margin: 6px 0;
  }
  .empty-icon {
    display: inline-flex;
    padding: 14px;
    border-radius: 14px;
    background: var(--accent-soft);
    color: var(--accent);
    margin-bottom: 8px;
  }
</style>
