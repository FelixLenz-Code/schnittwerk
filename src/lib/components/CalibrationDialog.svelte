<script lang="ts">
  // Screen calibration (menu Ansicht → Bildschirm kalibrieren …): hold a
  // bank card (ID-1, 85.6 × 53.98 mm) against the screen and match the
  // drawn card. Stored for the current monitor.
  import { DEFAULT_PX_PER_MM, editor } from "../editor.svelte";
  import { decimalComma, t } from "../i18n.svelte";
  import { resetCalibration, saveCalibration, screen } from "../screen.svelte";
  import Icon from "./Icon.svelte";

  const CARD_W = 85.6;
  const CARD_H = 53.98;
  const MIN = 2;
  const MAX = 5.6;

  /** Working value; only "Save" stores it. */
  let px = $state(editor.screenPxPerMm);
  const dpi = $derived(Math.round(px * 25.4));
  const fmt = (v: number) => (decimalComma() ? v.toFixed(2).replace(".", ",") : v.toFixed(2));

  function set(v: number) {
    px = Math.min(MAX, Math.max(MIN, Math.round(v * 1000) / 1000));
  }
  const close = () => (screen.dialogOpen = false);
  function save() {
    saveCalibration(px);
    editor.toast(t("Bildschirm kalibriert – 100 % entspricht jetzt der echten Größe."), "ok");
    close();
  }
  function reset() {
    resetCalibration();
    px = DEFAULT_PX_PER_MM;
  }
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === "Escape") close();
    else if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
      if ((e.target as HTMLElement).tagName === "INPUT") return;
      set(px + (e.key === "ArrowRight" ? 1 : -1) * (e.shiftKey ? 0.1 : 0.01));
      e.preventDefault();
    }
  }}
/>

<div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && close()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label={t("Bildschirm kalibrieren")}>
    <header>
      <h2><Icon name="fit" size={18} /> {t("Bildschirm kalibrieren")}</h2>
      <button class="btn icon" onclick={close} aria-label={t("Schließen")}><Icon name="x" /></button>
    </header>
    <div class="body">
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
      <div class="facts">
        <span>{t("{v} Pixel pro mm", { v: fmt(px) })} · {dpi} dpi</span>
        <span>{screen.label ? t("Gilt für: {m}", { m: screen.label }) : ""}</span>
      </div>
      <p class="hint">{t("Pfeiltasten ← → stellen fein ein (mit Umschalt gröber). Jeder Bildschirm wird einzeln kalibriert.")}</p>
    </div>
    <footer>
      <button class="btn" disabled={!screen.calibrated && Math.abs(px - DEFAULT_PX_PER_MM) < 0.001} onclick={reset}>{t("Zurücksetzen")}</button>
      <span class="spacer"></span>
      <button class="btn outline" onclick={close}>{t("Abbrechen")}</button>
      <button class="btn primary" onclick={save}><Icon name="check" /> {t("Speichern")}</button>
    </footer>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 70;
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
    width: min(560px, 100%);
    max-height: 100%;
    border-radius: 14px;
    background: var(--panel);
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.35);
    overflow: hidden;
  }
  header,
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 16px;
  }
  header {
    justify-content: space-between;
    border-bottom: 1px solid var(--border);
  }
  footer {
    border-top: 1px solid var(--border);
  }
  .spacer {
    flex: 1;
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    font-size: 15px;
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 16px;
    overflow-y: auto;
  }
  p {
    margin: 0;
    color: var(--text-2);
    line-height: 1.45;
  }
  .stage {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 200px;
    padding: 12px 0;
    overflow: hidden;
    border-radius: var(--radius-sm);
    background: var(--canvas);
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
  .facts {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    color: var(--text-3);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
  .hint {
    color: var(--text-3);
    font-size: 12px;
  }
</style>
