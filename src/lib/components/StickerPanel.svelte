<script lang="ts">
  import { api } from "../api";
  import { editor } from "../editor.svelte";
  import { uid } from "../geometry";
  import { layerToPath } from "../place";
  import type { ImageObj, StickerParams } from "../types";
  import Icon from "./Icon.svelte";
  import Segmented from "./Segmented.svelte";
  import Slider from "./Slider.svelte";
  import Toggle from "./Toggle.svelte";

  let maskKind = $state<"background" | "alpha">("background");
  let tolerance = $state(12);
  let marginMm = $state(3);
  let smoothness = $state(60);
  let fillHoles = $state(true);
  let colored = $state(false);
  let borderColor = $state("#ffffff");
  let bleedOn = $state(false);
  let bleedMm = $state(1.5);
  let applying = $state(false);

  const target = $derived.by((): ImageObj | null => {
    const imgs = editor.selected.filter((o): o is ImageObj => o.type === "image");
    return imgs.length === 1 ? imgs[0] : null;
  });
  const asset = $derived(target ? editor.images[target.imageId] : null);

  const params = $derived.by((): StickerParams | null => {
    if (!target || !asset) return null;
    return {
      mask: maskKind === "alpha" ? { kind: "alpha" } : { kind: "background", tolerance },
      offsetPx: (marginMm * asset.pxW) / target.w,
      smoothness,
      fillHoles,
      despeckle: 30,
    };
  });

  let seq = 0;
  $effect(() => {
    const p = params ? (JSON.parse(JSON.stringify(params)) as StickerParams) : null;
    const obj = target;
    if (!p || !obj) {
      editor.tracePreview = null;
      return;
    }
    const my = ++seq;
    const t = setTimeout(async () => {
      editor.tracing = true;
      const t0 = performance.now();
      try {
        const result = await api.stickerOutline(obj.imageId, p);
        if (my === seq) editor.tracePreview = { objId: obj.id, result, ms: Math.round(performance.now() - t0) };
      } catch (e) {
        if (my === seq) editor.error(e);
      } finally {
        if (my === seq) editor.tracing = false;
      }
    }, 150);
    return () => clearTimeout(t);
  });

  $effect(() => () => {
    seq++;
    editor.tracePreview = null;
  });

  const preview = $derived(editor.tracePreview && target && editor.tracePreview.objId === target.id ? editor.tracePreview : null);

  async function apply() {
    if (!preview || !target || !asset || !preview.result.layers.length || !params) return;
    const img = target;
    // Bleed first (async), then apply every change as one undo step.
    let bled: { id: string; src: string; name: string; pxW: number; pxH: number; pad: number } | null = null;
    if (bleedOn) {
      applying = true;
      try {
        const px = ((marginMm + bleedMm) * asset.pxW) / img.w;
        const r = await api.bleedImage(img.imageId, params.mask, px);
        bled = { id: r.info.id, src: r.info.src, name: asset.name, pxW: r.info.width, pxH: r.info.height, pad: r.pad };
      } catch (e) {
        editor.error(e);
        return;
      } finally {
        applying = false;
      }
    }
    const group = img.group ?? uid();
    const contour = layerToPath(img, preview.result.width, preview.result.height, preview.result.layers[0], {
      name: `${img.name} · Sticker-Kontur`,
      printFill: colored ? borderColor : null,
      group,
    });
    editor.checkpoint();
    const idx = editor.objects.findIndex((o) => o.id === img.id);
    const live = editor.objects[idx];
    live.group = group;
    if (bled && live.type === "image") {
      editor.images[bled.id] = { src: bled.src, name: bled.name, pxW: bled.pxW, pxH: bled.pxH };
      const cx = live.x + live.w / 2, cy = live.y + live.h / 2;
      const k = live.w / asset.pxW;
      live.imageId = bled.id;
      live.w = bled.pxW * k;
      live.h = bled.pxH * (live.h / asset.pxH);
      live.x = cx - live.w / 2;
      live.y = cy - live.h / 2;
    }
    // Contour goes below the image so a coloured border is printed underneath.
    editor.objects.splice(idx, 0, contour);
    editor.selection = [contour.id, img.id];
    editor.toast("Sticker-Kontur erstellt", "ok");
    editor.rightTab = "object";
  }
</script>

{#if !target || !asset}
  <div class="empty">
    <div class="empty-icon"><Icon name="sparkle" size={28} stroke={1.5} /></div>
    <p><strong>Motiv auswählen</strong></p>
    <p class="muted">Wähle ein importiertes Bild aus. Es bekommt eine Schnittkontur mit Rand – fertig für Print & Cut.</p>
  </div>
{:else}
  <div class="panel">
    {#if !editor.printCut.enabled}
      <div class="banner">
        <Icon name="sparkle" />
        <div>
          <strong>Print & Cut ist aus.</strong> Für Sticker die Seite mit Passermarken einschalten.
          <button class="btn outline small" onclick={() => editor.setPrintCut(true)}>Print & Cut aktivieren</button>
        </div>
      </div>
    {/if}

    <section>
      <h3 class="section-title">Motiv freistellen</h3>
      <Segmented
        value={maskKind}
        onchange={(v) => (maskKind = v)}
        options={[
          { value: "background", label: "Hintergrund entfernen", hint: "Für JPG/PNG mit einfarbigem Hintergrund – die Randfarbe wird ausgeblendet" },
          { value: "alpha", label: "Transparenz", hint: "Für PNG mit transparentem Hintergrund" },
        ]}
      />
      <div class="controls">
        {#if maskKind === "background"}
          <Slider label="Toleranz" bind:value={tolerance} min={2} max={50} unit=" %" hint="Wie stark eine Farbe vom Hintergrund abweichen muss" />
        {/if}
        <Toggle label="Löcher füllen" bind:checked={fillHoles} hint="Sticker als ein Stück – Innenflächen werden nicht ausgeschnitten" />
      </div>
    </section>

    <section>
      <h3 class="section-title">Kontur</h3>
      <div class="controls">
        <Slider label="Rand" bind:value={marginMm} min={0} max={10} step={0.5} unit=" mm" hint="Abstand der Schnittlinie zum Motiv" />
        <Slider label="Glättung" bind:value={smoothness} min={0} max={100} />
        <Toggle label="Rand farbig drucken" bind:checked={colored} hint="Füllt die Fläche innerhalb der Kontur – sonst bleibt das Papier weiß" />
        <Toggle label="Bleed (Farbe über die Kontur)" bind:checked={bleedOn} hint="Verlängert die Randfarben des Motivs nach außen – kein weißer Blitzer, wenn der Schnitt minimal daneben liegt" />
        {#if bleedOn}
          <Slider label="Bleed über die Kontur" bind:value={bleedMm} min={0.5} max={3} step={0.5} unit=" mm" />
        {/if}
        {#if colored}
          <label class="color-row">
            <span>Randfarbe</span>
            <input type="color" bind:value={borderColor} />
            <span class="mono">{borderColor}</span>
          </label>
        {/if}
      </div>
    </section>

    <div class="footer">
      <div class="stats">
        {#if editor.tracing}
          <span class="dot busy"></span> Berechne …
        {:else if preview}
          <span class="dot"></span>
          {preview.result.layers[0]?.contours ?? 0}
          {(preview.result.layers[0]?.contours ?? 0) === 1 ? "Kontur" : "Konturen"} · {preview.ms} ms
        {:else}
          –
        {/if}
      </div>
      {#if preview && (preview.result.layers[0]?.contours ?? 0) > 1}
        <p class="hint">Mehrere Teile – größerer Rand verbindet sie zu einem Sticker.</p>
      {/if}
      <button class="btn primary grow" disabled={!preview?.result.layers.length || applying} onclick={apply}><Icon name="check" /> {applying ? "Erzeuge Bleed …" : "Kontur übernehmen"}</button>
    </div>
  </div>
{/if}

<style>
  .panel {
    display: flex;
    flex-direction: column;
    min-height: 100%;
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
  .banner {
    display: flex;
    gap: 10px;
    margin: 12px 16px 0;
    padding: 10px 12px;
    border-radius: var(--radius);
    background: var(--accent-soft);
    color: var(--text);
    line-height: 1.45;
  }
  .banner :global(svg) {
    color: var(--accent);
    flex: none;
    margin-top: 2px;
  }
  .small {
    height: 26px;
    margin-top: 8px;
  }
  .color-row {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--text-2);
  }
  .color-row input {
    width: 30px;
    height: 26px;
    padding: 0;
    border: 0;
    background: none;
  }
  .mono {
    font-family: ui-monospace, monospace;
    color: var(--text-3);
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
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--ok);
  }
  .dot.busy {
    background: var(--warn);
  }
  .hint {
    margin: 0;
    color: var(--text-3);
    font-size: 12px;
  }
  .grow {
    justify-content: center;
  }
  .muted {
    color: var(--text-3);
    margin: 0;
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
