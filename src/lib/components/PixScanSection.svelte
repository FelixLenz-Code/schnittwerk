<script lang="ts">
  import { editor } from "../editor.svelte";
  import { perspective } from "../perspective.svelte";
  import { pixscan } from "../pixscan.svelte";
  import type { ImageObj } from "../types";
  import Icon from "./Icon.svelte";

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
  const f = (v: number) => (v >= 0 ? "+" : "") + v.toFixed(2).replace(".", ",");
</script>

<section>
  <h3 class="section-title">PixScan-Matte</h3>
  {#if corners}
    <p class="muted">Die vier Punkte auf die Ecken des schwarzen Innenrahmens ziehen. Der gefüllte Punkt gehört an die Ecke beim großen Quadrat.</p>
    <div class="row gap">
      <button class="btn outline" onclick={() => perspective.cancel()} disabled={perspective.busy}>Abbrechen</button>
      <button class="btn outline" onclick={() => perspective.rotate()} disabled={perspective.busy} title="Gefüllten Punkt an die nächste Ecke setzen">Ecke 1 weiter</button>
      <button class="btn primary grow" onclick={() => perspective.apply()} disabled={perspective.busy}><Icon name="check" /> Ausrichten</button>
    </div>
  {:else if live && placed}
    <p class="muted">Das Foto liegt in Originalgröße dort, wo der Plotter schneidet. Motiv tracen oder Schnittlinien darauf zeichnen und ganz normal schneiden.</p>
    <div class="row gap">
      <button class="btn outline" onclick={() => live && pixscan.turn(live)} disabled={pixscan.busy || !pixscan.canEdit(live)} title="Falls die Matte verkehrt herum erkannt wurde">Um 180° drehen</button>
      <button class="btn outline" onclick={() => live && pixscan.editCorners(live)} disabled={pixscan.busy || !pixscan.canEdit(live)} title="Erkannte Rahmenecken von Hand nachziehen">Ecken anpassen</button>
    </div>
    <details bind:open={showCalib}>
      <summary>Kalibrieren mit diesem Foto</summary>
      <p class="muted">Foto verschieben, bis die gezeichneten Kreuze genau unter den grünen Testkreuzen liegen. Dann übernehmen.</p>
      <div class="nudge">
        <span></span>
        <button class="btn outline" onclick={() => live && pixscan.nudge(live, 0, -step)} aria-label="Nach oben">↑</button>
        <span></span>
        <button class="btn outline" onclick={() => live && pixscan.nudge(live, -step, 0)} aria-label="Nach links">←</button>
        <select class="field" bind:value={step} title="Schrittweite">
          <option value={0.1}>0,1 mm</option>
          <option value={0.2}>0,2 mm</option>
          <option value={0.5}>0,5 mm</option>
          <option value={1}>1 mm</option>
        </select>
        <button class="btn outline" onclick={() => live && pixscan.nudge(live, step, 0)} aria-label="Nach rechts">→</button>
        <span></span>
        <button class="btn outline" onclick={() => live && pixscan.nudge(live, 0, step)} aria-label="Nach unten">↓</button>
        <span></span>
      </div>
      {#if shift}
        <p class="muted center">Verschoben: X {f(shift.dx)} mm · Y {f(shift.dy)} mm</p>
      {/if}
      <button class="btn primary wide" onclick={() => live && pixscan.applyCalibration(live)} disabled={!moved}>
        <Icon name="check" /> Kalibrierung übernehmen
      </button>
    </details>
  {:else if live}
    <p class="muted">Foto der PixScan-Matte mit Material darauf: Rahmen erkennen, gerade ziehen und in Originalgröße auf die Matte legen.</p>
    <div class="row">
      <button class="btn outline" onclick={() => live && pixscan.align(live)} disabled={pixscan.busy}>
        {pixscan.busy ? "Suche Rahmen …" : "Als PixScan-Foto ausrichten"}
      </button>
    </div>
  {/if}

  <details class="calib">
    <summary>Kalibrierung</summary>
    <p class="muted">
      Einmalig: Papier auf die PixScan-Matte legen, Testkreuze anlegen und mit dem Stift zeichnen. Dann die Matte fotografieren, das Foto ausrichten und
      unter „Kalibrieren mit diesem Foto“ angleichen.
    </p>
    <button class="btn outline wide" onclick={() => pixscan.addTestCrosses()}>Testkreuze anlegen</button>
    <div class="offset">
      <span>Rahmen auf der Matte</span>
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
