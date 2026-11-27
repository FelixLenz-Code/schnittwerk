<script lang="ts">
  // Side panel when nothing is selected: the work surface and tools that act
  // on the whole sheet.
  import { addWeeding, frameSelection, nestSelection } from "../actions";
  import { editor } from "../editor.svelte";
  import { t } from "../i18n.svelte";
  import { importDialog } from "../io.svelte";
  import Icon from "./Icon.svelte";
  import PixScanSection from "./PixScanSection.svelte";
  import Section from "./Section.svelte";
  import SurfacePicker from "./SurfacePicker.svelte";

  const visible = $derived(editor.objects.filter((o) => o.visible && !o.locked));
  const hasPaths = $derived(visible.some((o) => o.type === "path" && o.cut !== false));
  let busy = $state(false);

  /** Runs a selection tool on everything, then leaves the selection empty. */
  async function onAll(fn: () => void | Promise<void>) {
    busy = true;
    editor.selection = visible.map((o) => o.id);
    try {
      await fn();
    } finally {
      editor.selection = [];
      busy = false;
    }
  }
</script>

<Section id="surface" title={t("Worauf schneidest du?")}>
  <SurfacePicker variant="inline" />
</Section>

{#if visible.length}
  <Section id="sheet" title={t("Für den ganzen Bogen")}>
    <div class="tools">
      <button class="tool" disabled={busy} onclick={() => onAll(() => nestSelection(false, 3))}>
        <Icon name="grid" />
        <span><b>{t("Platzsparend anordnen")}</b><small>{t("Alle Teile verschachteln, damit weniger Material übrig bleibt")}</small></span>
      </button>
      <button class="tool" disabled={busy} onclick={() => onAll(frameSelection)}>
        <Icon name="square" />
        <span><b>{t("Schnittrahmen um alles")}</b><small>{t("Abgerundeter Rahmen, wird durchgeschnitten")}</small></span>
      </button>
      {#if hasPaths}
        <button class="tool" disabled={busy} onclick={() => onAll(() => addWeeding({ margin: 3, spacing: 20, dir: "h" }))}>
          <Icon name="vinyl" />
          <span><b>{t("Entgitterungshilfe um alles")}</b><small>{t("Rahmen und Linien durch den Abfall – Folie lässt sich in Streifen abziehen")}</small></span>
        </button>
      {/if}
    </div>
    <p class="muted small">{t("Für einzelne Teile: Objekt auswählen – die Werkzeuge stehen dann unter „Werkzeuge“.")}</p>
  </Section>
{:else}
  <section class="start">
    <p><strong>{t("Noch leer")}</strong></p>
    <p class="muted">{t("Importiere ein Bild oder zeichne links eine Form.")}</p>
    <button class="btn primary" onclick={importDialog}><Icon name="imagePlus" /> {t("Importieren")}</button>
  </section>
{/if}

<Section id="pixscan-page" title={t("PixScan-Matte")} open={false}>
  <div class="pix"><PixScanSection img={null} /></div>
</Section>

<Section id="keys" title={t("Tastenkürzel")} open={false}>
  <div class="tips">
    <div><kbd>V</kbd> {t("Auswahl")}</div>
    <div><kbd>H</kbd> / <kbd>{t("Leertaste")}</kbd> {t("Verschieben")}</div>
    <div><kbd>R</kbd> {t("Rechteck")} · <kbd>E</kbd> {t("Ellipse")} · <kbd>P</kbd> {t("Vieleck")} · <kbd>S</kbd> {t("Stern")}</div>
    <div><kbd>L</kbd> {t("Linie")} · <kbd>F</kbd> {t("Freihand")} · <kbd>T</kbd> {t("Text")}</div>
    <div>{t("Hilfslinien aus den Linealen ziehen")}</div>
    <div>{t("Mausrad = Zoom")} · <kbd>{t("Strg")}</kbd>+<kbd>0</kbd> {t("Einpassen")}</div>
  </div>
</Section>

<style>
  .tools {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .tool {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 9px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--panel);
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .tool:hover:not(:disabled) {
    border-color: var(--border-strong);
    background: var(--panel-2);
  }
  .tool:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .tool :global(svg) {
    flex: none;
    margin-top: 1px;
    color: var(--accent);
  }
  .tool span {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .tool b {
    font-weight: 600;
  }
  .tool small {
    color: var(--text-3);
    font-size: 11.5px;
    line-height: 1.35;
  }
  .muted {
    color: var(--text-3);
  }
  .small {
    margin: 10px 0 0;
    font-size: 12px;
  }
  .start {
    padding: 20px 16px;
    border-bottom: 1px solid var(--border);
    color: var(--text-2);
  }
  .start p {
    margin: 0 0 6px;
  }
  .start .btn {
    margin-top: 6px;
  }
  /* PixScanSection brings its own section frame and title. */
  .pix :global(section) {
    padding: 0;
    border: 0;
  }
  .pix :global(section > .section-title) {
    display: none;
  }
  .tips {
    display: flex;
    flex-direction: column;
    gap: 6px;
    color: var(--text-2);
    font-size: 12.5px;
  }
  kbd {
    font: inherit;
    font-size: 11px;
    padding: 0 4px;
    border: 1px solid var(--border);
    border-radius: 4px;
  }
</style>
