<script lang="ts">
  import { api } from "../api";
  import { editor } from "../editor.svelte";
  import { uid } from "../geometry";
  import { layerToPath } from "../place";
  import type { ImageObj, Obj, StickerParams, TraceResult } from "../types";
  import { arrangeUnits } from "../actions";
  import Icon from "./Icon.svelte";
  import { t } from "../i18n.svelte";
  import Segmented from "./Segmented.svelte";
  import Slider from "./Slider.svelte";
  import Toggle from "./Toggle.svelte";
  import { ui } from "../ui.svelte";

  let maskKind = $state<"background" | "alpha">("background");
  let tolerance = $state(12);
  let marginMm = $state(3);
  let smoothness = $state(60);
  let fillHoles = $state(true);
  let colored = $state(false);
  let borderColor = $state("#ffffff");
  let bleedOn = $state(false);
  /** Kiss-cut the sticker and through-cut a backing outline around it. */
  let dieCut = $state(editor.stickerStyle === "die");
  let backingMm = $state(2);
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

  const KISS = "#e5484d";
  const THROUGH = "#3e63dd";

  let seq = 0;
  $effect(() => {
    const p = params ? (JSON.parse(JSON.stringify(params)) as StickerParams) : null;
    void dieCut;
    void backingMm;
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
        // Kiss-cut contour, plus a wider through-cut outline for the backing.
        const extraPx = (backingMm * (editor.images[obj.imageId]?.pxW ?? 1)) / obj.w;
        const [kiss, through] = await Promise.all([
          api.stickerOutline(obj.imageId, p),
          dieCut ? api.stickerOutline(obj.imageId, { ...p, offsetPx: p.offsetPx + extraPx }) : Promise.resolve(null),
        ]);
        const layers = [
          ...kiss.layers.slice(0, 1).map((l) => ({ ...l, color: KISS })),
          ...(through?.layers.slice(0, 1).map((l) => ({ ...l, color: THROUGH })) ?? []),
        ];
        const result = { ...kiss, layers };
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
    editor.checkpoint("Sticker-Kontur");
    const ids = insertSticker(img, asset, preview.result, bled);
    editor.selection = ids;
    editor.toast(preview.result.layers[1] ? "Sticker angelegt: rot wird angeschnitten, blau durchgeschnitten" : "Sticker-Kontur erstellt", "ok");
    editor.rightTab = "object";
    ui.nextSteps = { kind: "sticker", sel: editor.selection.join(",") };
  }

  type Bled = { id: string; src: string; name: string; pxW: number; pxH: number; pad: number };

  /**
   * Puts the contour (layer 0, kiss cut) and the backing (layer 1, through
   * cut) of a sticker under its picture and groups them. No undo step –
   * the caller makes one. Returns the ids of the sticker's objects.
   */
  function insertSticker(img: ImageObj, asset: { pxW: number; pxH: number }, r: TraceResult, bled: Bled | null): string[] {
    const group = img.group ?? uid();
    const contour = layerToPath(img, r.width, r.height, r.layers[0], {
      name: `${img.name} · ${t("Anschneiden")}`,
      color: KISS,
      printFill: colored ? borderColor : null,
      group,
    });
    const backing = r.layers[1]
      ? layerToPath(img, r.width, r.height, r.layers[1], { name: `${img.name} · ${t("Durchschneiden")}`, color: THROUGH, group })
      : null;
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
    // Contours go below the image so a coloured border is printed underneath.
    editor.objects.splice(idx, 0, ...(backing ? [backing, contour] : [contour]));
    return [...(backing ? [backing.id] : []), contour.id, img.id];
  }

  // ---------- sticker sheet from several pictures ----------
  /** Selected pictures without a sticker contour yet (several = sheet mode). */
  const batch = $derived(
    editor.selected.filter(
      (o): o is ImageObj => o.type === "image" && !(o.group && editor.objects.some((p) => p.type === "path" && p.group === o.group)),
    ),
  );
  let copies = $state(1);
  let arrange = $state(true);
  let batchBusy = $state(false);
  let batchDone = $state(0);

  /** Contours for all selected pictures, copies, then arranged on the sheet. */
  async function makeSheet() {
    const imgs = $state.snapshot(batch) as ImageObj[];
    if (!imgs.length) return;
    batchBusy = true;
    batchDone = 0;
    try {
      const results: { img: ImageObj; asset: { pxW: number; pxH: number; name: string }; r: TraceResult; bled: Bled | null }[] = [];
      for (const img of imgs) {
        const asset = editor.images[img.imageId];
        if (!asset) continue;
        const p: StickerParams = {
          mask: maskKind === "alpha" ? { kind: "alpha" } : { kind: "background", tolerance },
          offsetPx: (marginMm * asset.pxW) / img.w,
          smoothness,
          fillHoles,
          despeckle: 30,
        };
        const extraPx = (backingMm * asset.pxW) / img.w;
        const [kiss, through] = await Promise.all([
          api.stickerOutline(img.imageId, p),
          dieCut ? api.stickerOutline(img.imageId, { ...p, offsetPx: p.offsetPx + extraPx }) : Promise.resolve(null),
        ]);
        if (!kiss.layers.length) continue;
        let bled: Bled | null = null;
        if (bleedOn) {
          const b = await api.bleedImage(img.imageId, p.mask, ((marginMm + bleedMm) * asset.pxW) / img.w);
          bled = { id: b.info.id, src: b.info.src, name: asset.name, pxW: b.info.width, pxH: b.info.height, pad: b.pad };
        }
        const layers = [kiss.layers[0], ...(through?.layers.slice(0, 1) ?? [])];
        results.push({ img, asset, r: { ...kiss, layers }, bled });
        batchDone++;
      }
      if (!results.length) {
        editor.toast("Keine Kontur gefunden – Einstellungen prüfen.", "error");
        return;
      }
      editor.checkpoint("Sticker-Bogen");
      const units: Obj[][] = [];
      for (const { img, asset, r, bled } of results) {
        const ids = insertSticker(img, asset, r, bled);
        const unit = editor.objects.filter((o) => ids.includes(o.id));
        units.push(unit);
        // Copies of the whole sticker (picture + contours), each its own group.
        for (let k = 1; k < copies; k++) {
          const g = uid();
          const dup = unit.map((o) => ({ ...($state.snapshot(o) as Obj), id: uid(), group: g }));
          editor.objects.push(...dup);
          units.push(editor.objects.slice(-dup.length));
        }
      }
      let msg = t("{n} Sticker angelegt", { n: units.length });
      if (arrange) {
        const r = await arrangeUnits(units, 3);
        if (r.unplaced) msg = t("{n} Sticker angelegt, {m} passen nicht mehr auf den Bogen und liegen rechts daneben", { n: units.length, m: r.unplaced });
      }
      editor.selection = units.flat().map((o) => o.id);
      editor.toast(msg, "ok");
      editor.rightTab = "object";
      ui.nextSteps = { kind: "sticker", sel: editor.selection.join(",") };
    } catch (e) {
      editor.error(e);
    } finally {
      batchBusy = false;
    }
  }
</script>

{#if !target && batch.length < 2}
  <div class="empty">
    <div class="empty-icon"><Icon name="sparkle" size={28} stroke={1.5} /></div>
    <p><strong>{t("Motiv auswählen")}</strong></p>
    <p class="muted">{t("Wähle ein importiertes Bild aus. Es bekommt eine Schnittkontur mit Rand – fertig für Print & Cut.")}</p>
  </div>
{:else}
  <div class="panel">
    {#if !editor.printCut.enabled}
      <div class="banner">
        <Icon name="sparkle" />
        <div>
          <strong>{t("Print & Cut ist aus.")}</strong> {t("Für Sticker die Seite mit Passermarken einschalten.")}
          <button class="btn outline small" onclick={() => editor.setPrintCut(true)}>{t("Print & Cut aktivieren")}</button>
        </div>
      </div>
    {/if}

    <section>
      <h3 class="section-title">{t("Motiv freistellen")}</h3>
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
      <h3 class="section-title">{t("Kontur")}</h3>
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
            <span>{t("Randfarbe")}</span>
            <input type="color" bind:value={borderColor} />
            <span class="mono">{borderColor}</span>
          </label>
        {/if}
      </div>
    </section>

    <section>
      <h3 class="section-title">{t("Schneiden")}</h3>
      <div class="controls">
        <Toggle
          label="Einzeln ausschneiden"
          bind:checked={dieCut}
          hint="Sticker wird angeschnitten (abziehbar) und mit etwas Trägerpapier komplett ausgeschnitten"
        />
        {#if dieCut}
          <Slider label="Trägerrand" bind:value={backingMm} min={1} max={6} step={0.5} unit=" mm" hint="Abstand der Durchschnitt-Linie zur Sticker-Kontur" />
          <p class="legend">
            <span class="sw" style:background={KISS}></span> {t("anschneiden – nur die Stickerfolie")}
            <br /><span class="sw" style:background={THROUGH}></span> {t("durchschneiden – samt Trägerpapier")}
          </p>
        {:else}
          <p class="legend"><span class="sw" style:background={KISS}></span> {t("nur anschneiden – die Sticker bleiben auf dem Bogen")}</p>
        {/if}
      </div>
    </section>

    {#if !target}
      <div class="footer">
        <h3 class="section-title">{t("Sticker-Bogen aus {n} Bildern", { n: batch.length })}</h3>
        <Slider label="Exemplare je Motiv" bind:value={copies} min={1} max={20} unit="×" />
        <Toggle label="Platzsparend auf dem Bogen anordnen" bind:checked={arrange} />
        <button class="btn primary grow" disabled={batchBusy} onclick={makeSheet}>
          <Icon name="sparkle" /> {batchBusy ? t("Erzeuge Konturen … {a}/{b}", { a: batchDone, b: batch.length }) : t("Sticker-Bogen erstellen")}
        </button>
        <p class="hint">{t("Alle ausgewählten Bilder bekommen mit den Einstellungen oben eine Kontur.")}</p>
      </div>
    {:else}
    <div class="footer">
      <div class="stats">
        {#if editor.tracing}
          <span class="dot busy"></span> {t("Berechne …")}
        {:else if preview}
          <span class="dot"></span>
          {(preview.result.layers[0]?.contours ?? 0) === 1 ? t("1 Kontur") : t("{n} Konturen", { n: preview.result.layers[0]?.contours ?? 0 })} · {preview.ms} ms
        {:else}
          –
        {/if}
      </div>
      {#if preview && (preview.result.layers[0]?.contours ?? 0) > 1}
        <p class="hint">{t("Mehrere Teile – größerer Rand verbindet sie zu einem Sticker.")}</p>
      {/if}
      <button class="btn primary grow" disabled={!preview?.result.layers.length || applying} onclick={apply}><Icon name="check" /> {applying ? t("Erzeuge Bleed …") : t("Kontur übernehmen")}</button>
    </div>
    {/if}
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
  .legend {
    margin: 0;
    color: var(--text-2);
    font-size: 12px;
    line-height: 1.7;
  }
  .sw {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 3px;
    margin-right: 4px;
    vertical-align: -1px;
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
