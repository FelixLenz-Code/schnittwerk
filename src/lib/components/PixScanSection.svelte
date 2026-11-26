<script lang="ts">
  import { editor } from "../editor.svelte";
  import { perspective } from "../perspective.svelte";
  import { pixscan } from "../pixscan.svelte";
  import type { ImageObj } from "../types";
  import Icon from "./Icon.svelte";
  import { decimalComma, t } from "../i18n.svelte";

  let { img }: { img: ImageObj | null } = $props();

  let step = $state(0.2);
  let showCalib = $state(false);

  // Read the live object so position changes show up immediately.
  const live = $derived.by((): ImageObj | null => {
    const o = img ? editor.byId(img.id) : undefined;
    return o?.type === "image" ? o : null;
  });
  const placed = $derived(!!live?.pixscan);
  const shift = $derived(live ? pixscan.shift(live) : null);
  const moved = $derived(!!shift && (Math.abs(shift.dx) > 0.001 || Math.abs(shift.dy) > 0.001));
  const corners = $derived(!!live && perspective.objId === live.id && !!perspective.custom);
  const f = (v: number) => (v >= 0 ? "+" : "") + (decimalComma() ? v.toFixed(2).replace(".", ",") : v.toFixed(2));
</script>

<section>
  <h3 class="section-title">{t("PixScan-Matte")}</h3>
  {#if corners}
    <p class="muted">{t("Die vier Punkte auf die Ecken des schwarzen Innenrahmens ziehen. Der gefüllte Punkt gehört an die Ecke beim großen Quadrat.")}</p>
    <div class="row gap">
      <button class="btn outline" onclick={() => perspective.cancel()} disabled={perspective.busy}>{t("Abbrechen")}</button>
      <button class="btn outline" onclick={() => perspective.rotate()} disabled={perspective.busy} title={t("Gefüllten Punkt an die nächste Ecke setzen")}>{t("Ecke 1 weiter")}</button>
      <button class="btn primary grow" onclick={() => perspective.apply()} disabled={perspective.busy}><Icon name="check" /> {t("Ausrichten")}</button>
    </div>
  {:else if live && placed}
    <p class="muted">{t("Das Foto liegt in Originalgröße dort, wo der Plotter schneidet. Motiv tracen oder Schnittlinien darauf zeichnen und ganz normal schneiden.")}</p>
    <div class="row gap">
      <button class="btn outline" onclick={() => live && pixscan.turn(live)} disabled={pixscan.busy || !pixscan.canEdit(live)} title={t("Falls die Matte verkehrt herum erkannt wurde")}>{t("Um 180° drehen")}</button>
      <button class="btn outline" onclick={() => live && pixscan.editCorners(live)} disabled={pixscan.busy || !pixscan.canEdit(live)} title={t("Erkannte Rahmenecken von Hand nachziehen")}>{t("Ecken anpassen")}</button>
    </div>
    <details bind:open={showCalib}>
      <summary>{t("Kalibrieren mit diesem Foto")}</summary>
      <p class="muted">{t("Foto verschieben, bis die gezeichneten Kreuze genau unter den grünen Testkreuzen liegen. Dann übernehmen.")}</p>
      <div class="nudge">
        <span></span>
        <button class="btn outline" onclick={() => live && pixscan.nudge(live, 0, -step)} aria-label={t("Nach oben")}>↑</button>
        <span></span>
        <button class="btn outline" onclick={() => live && pixscan.nudge(live, -step, 0)} aria-label={t("Nach links")}>←</button>
        <select class="field" bind:value={step} title={t("Schrittweite")}>
          <option value={0.1}>0,1 mm</option>
          <option value={0.2}>0,2 mm</option>
          <option value={0.5}>0,5 mm</option>
          <option value={1}>1 mm</option>
        </select>
        <button class="btn outline" onclick={() => live && pixscan.nudge(live, step, 0)} aria-label={t("Nach rechts")}>→</button>
        <span></span>
        <button class="btn outline" onclick={() => live && pixscan.nudge(live, 0, step)} aria-label={t("Nach unten")}>↓</button>
        <span></span>
      </div>
      {#if shift}
        <p class="muted center">{t("Verschoben: X {x} mm · Y {y} mm", { x: f(shift.dx), y: f(shift.dy) })}</p>
      {/if}
      <button class="btn primary wide" onclick={() => live && pixscan.applyCalibration(live)} disabled={!moved}>
        <Icon name="check" /> {t("Kalibrierung übernehmen")}
      </button>
    </details>
  {:else if live}
    <p class="muted">{t("Foto der PixScan-Matte mit Material darauf: Rahmen erkennen, gerade ziehen und in Originalgröße auf die Matte legen.")}</p>
    <div class="row">
      <button class="btn outline" onclick={() => live && pixscan.align(live)} disabled={pixscan.busy}>
        {pixscan.busy ? t("Suche Rahmen …") : t("Als PixScan-Foto ausrichten")}
      </button>
    </div>
  {/if}

  <details class="calib">
    <summary>{t("Kalibrierung")}</summary>
    <p class="muted">
      {t("Einmalig: Papier auf die PixScan-Matte legen, Testkreuze anlegen und mit dem Stift zeichnen. Dann die Matte fotografieren, das Foto ausrichten und unter „Kalibrieren mit diesem Foto“ angleichen.")}
    </p>
    <button class="btn outline wide" onclick={() => pixscan.addTestCrosses()}>{t("Testkreuze anlegen")}</button>
    <div class="offset">
      <span>{t("Rahmen auf der Matte")}</span>
      <label>X <input class="field num" type="number" step="0.1" bind:value={pixscan.offset.x} onchange={() => pixscan.save()} /> mm</label>
      <label>Y <input class="field num" type="number" step="0.1" bind:value={pixscan.offset.y} onchange={() => pixscan.save()} /> mm</label>
    </div>
  </details>
</section>

<style>
  section {
    padding: 14px 16px;
    border-bottom: 1px solid var(--border);
  }
  .muted {
    color: var(--text-3);
    margin: 0 0 10px;
  }
  .center {
    text-align: center;
    margin-top: 8px;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
  }
  .gap {
    gap: 6px;
  }
  .grow {
    flex: 1;
    justify-content: center;
  }
  .wide {
    width: 100%;
    justify-content: center;
  }
  details {
    margin-top: 12px;
  }
  summary {
    cursor: pointer;
    color: var(--text-2);
    font-weight: 600;
    margin-bottom: 8px;
  }
  .nudge {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    gap: 4px;
    justify-items: center;
    align-items: center;
    max-width: 220px;
    margin: 0 auto;
  }
  .nudge .btn {
    min-width: 36px;
    justify-content: center;
  }
  .nudge select {
    width: 84px;
  }
  .offset {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 12px;
    align-items: center;
    margin-top: 10px;
  }
  .offset > span {
    width: 100%;
    color: var(--text-2);
  }
  .offset label {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .offset input {
    width: 72px;
  }
</style>
