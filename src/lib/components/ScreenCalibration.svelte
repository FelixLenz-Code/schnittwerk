<script lang="ts">
  // Real size on screen: hold a bank card (ID-1, 85.6 × 53.98 mm) against
  // the screen and match the drawn card. Sets editor.screenPxPerMm.
  import { DEFAULT_PX_PER_MM, editor } from "../editor.svelte";
  import { decimalComma, t } from "../i18n.svelte";

  const CARD_W = 85.6;
  const CARD_H = 53.98;
  const MIN = 2;
  const MAX = 5.2;

  let open = $state(false);
  const px = $derived(editor.screenPxPerMm);
  const calibrated = $derived(Math.abs(px - DEFAULT_PX_PER_MM) > 0.005);
  const dpi = $derived(Math.round(px * 25.4));

  function set(v: number) {
    editor.screenPxPerMm = Math.min(MAX, Math.max(MIN, Math.round(v * 1000) / 1000));
  }
  const fmt = (v: number) => (decimalComma() ? v.toFixed(2).replace(".", ",") : v.toFixed(2));
</script>

<div class="row">
  <span>{t("Bildschirm für 1:1")}</span>
  <span class="state">
    {calibrated ? t("kalibriert ({d} dpi)", { d: dpi }) : t("Standard (96 dpi)")}
    <button class="btn small" onclick={() => (open = !open)}>{open ? t("Fertig") : t("Kalibrieren …")}</button>
  </span>
</div>

{#if open}
  <div class="calib">
    <p>{t("Halte eine Bankkarte (85,6 × 54 mm) an den Bildschirm und stelle die Breite so ein, dass der Rahmen genau so groß ist wie die Karte.")}</p>
    <div class="stage">
      <div class="card" style:width="{CARD_W * px}px" style:height="{CARD_H * px}px" style:border-radius="{3 * px}px">
        <span>85,6 mm</span>
      </div>
    </div>
    <div class="controls">
      <button class="btn outline" onclick={() => set(px - 0.05)} aria-label={t("Kleiner")}>−</button>
      <input type="range" min={MIN} max={MAX} step="0.005" value={px} oninput={(e) => set(+e.currentTarget.value)} aria-label={t("Bildschirmpixel pro Millimeter")} />
      <button class="btn outline" onclick={() => set(px + 0.05)} aria-label={t("Größer")}>+</button>
    </div>
    <div class="foot">
      <span class="muted">{t("{v} Pixel pro mm", { v: fmt(px) })}</span>
      <button class="btn small" disabled={!calibrated} onclick={() => set(DEFAULT_PX_PER_MM)}>{t("Zurücksetzen")}</button>
    </div>
  </div>
{/if}

<style>
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    color: var(--text-2);
  }
  .state {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
  }
  .small {
    height: 26px;
    border: 1px solid var(--border);
    font-size: 12px;
  }
  .calib {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--panel-2);
  }
  .calib p {
    margin: 0;
    color: var(--text-2);
    font-size: 12.5px;
    line-height: 1.45;
  }
  .stage {
    display: flex;
    justify-content: center;
    overflow: hidden;
  }
  .card {
    position: relative;
    flex: none;
    border: 2px solid var(--accent);
    background: repeating-linear-gradient(45deg, var(--accent-soft) 0 8px, transparent 8px 16px);
  }
  .card span {
    position: absolute;
    left: 50%;
    bottom: 6px;
    transform: translateX(-50%);
    color: var(--accent);
    font-size: 11px;
    font-weight: 600;
  }
  .controls {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .controls input {
    flex: 1;
    accent-color: var(--accent);
  }
  .controls .btn {
    width: 30px;
    justify-content: center;
  }
  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .muted {
    color: var(--text-3);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
</style>
