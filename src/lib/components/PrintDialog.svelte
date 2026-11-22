<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import { api } from "../api";
  import { editor, PAGES } from "../editor.svelte";
  import { plotter } from "../plotter.svelte";
  import { renderPage } from "../print";
  import Icon from "./Icon.svelte";

  let printers = $state<string[]>([]);
  let printer = $state<string>("");
  let preview = $state<string | null>(null);
  let busy = $state<"print" | "pdf" | null>(null);

  const page = $derived(PAGES[editor.printCut.page]);
  const hasPrintable = $derived(editor.objects.some((o) => o.visible && (o.type === "image" || (o.type === "path" && o.printFill))));

  onMount(async () => {
    try {
      const p = await api.listPrinters();
      printers = p.names;
      printer = p.default ?? p.names[0] ?? "";
    } catch (e) {
      editor.error(e);
    }
  });

  $effect(() => {
    // Re-render the preview when the mark position changes.
    void editor.printCut.insetX;
    void editor.printCut.insetY;
    renderPage(60).then((p) => (preview = p));
  });

  async function doPrint(toPdf: boolean) {
    let path: string | null = null;
    if (toPdf) {
      path = await save({ defaultPath: "Print-and-Cut.pdf", filters: [{ name: "PDF", extensions: ["pdf"] }] });
      if (!path) return;
      if (!path.toLowerCase().endsWith(".pdf")) path += ".pdf";
    }
    busy = toPdf ? "pdf" : "print";
    try {
      const png = await renderPage(300);
      await api.printPage(png, editor.mat.w, editor.mat.h, page.media, toPdf ? null : printer || null, path);
      editor.toast(toPdf ? "PDF gespeichert" : "An den Drucker gesendet", "ok");
      plotter.printOpen = false;
    } catch (e) {
      editor.error(e);
    } finally {
      busy = null;
    }
  }

  function close() {
    if (!busy) plotter.printOpen = false;
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

<div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && close()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label="Drucken">
    <header>
      <h2><Icon name="printer" size={18} /> Drucken</h2>
      <button class="btn icon" onclick={close} aria-label="Schließen"><Icon name="x" /></button>
    </header>

    {#if !editor.printCut.enabled}
      <div class="body single">
        <p>Zum Drucken für Print & Cut wird die Seite mit Passermarken benötigt.</p>
        <button class="btn primary" onclick={() => editor.setPrintCut(true)}>Print & Cut aktivieren</button>
      </div>
    {:else}
      <div class="body">
        <div class="preview">
          {#if preview}
            <img src={preview} alt="Druckvorschau" />
          {:else}
            <div class="ph">Vorschau …</div>
          {/if}
        </div>
        <div class="side">
          <label class="field-row">
            <span>Drucker</span>
            <select class="field" bind:value={printer}>
              {#each printers as p}<option value={p}>{p}</option>{/each}
              {#if !printers.length}<option value="">Kein Drucker gefunden</option>{/if}
            </select>
          </label>
          <div class="insets">
            <span>Abstand der Passermarken zum Papierrand</span>
            <div class="inset-row">
              <label>links/rechts <input class="field num" type="number" min="5" max="40" step="1" bind:value={editor.printCut.insetX} /> mm</label>
              <label>oben/unten <input class="field num" type="number" min="5" max="40" step="1" bind:value={editor.printCut.insetY} /> mm</label>
            </div>
          </div>
          <div class="facts">
            <div><span>Papier</span><b>{editor.printCut.page} ({page.w} × {page.h} mm)</b></div>
            <div><span>Auflösung</span><b>300 dpi</b></div>
            <div><span>Skalierung</span><b>aus (100 %)</b></div>
          </div>
          <p class="note">
            Wichtig: In Druckdialogen niemals „An Seite anpassen“ wählen – sonst passen Druck und Schnitt nicht zusammen. Schnittwerk schaltet die Skalierung beim Drucken automatisch ab.
          </p>
          {#if !hasPrintable}
            <p class="warn">Auf der Seite ist nichts zum Drucken – es würden nur die Passermarken gedruckt.</p>
          {/if}
          <div class="actions">
            <button class="btn outline" disabled={!!busy} onclick={() => doPrint(true)}>
              <Icon name="download" /> {busy === "pdf" ? "Erstelle PDF …" : "Als PDF"}
            </button>
            <button class="btn primary grow" disabled={!!busy || !printers.length} onclick={() => doPrint(false)}>
              <Icon name="printer" /> {busy === "print" ? "Sende …" : "Drucken"}
            </button>
          </div>
          <p class="hint">Danach den Bogen oben links auf die Matte legen, laden und im Schneiden-Dialog starten.</p>
        </div>
      </div>
    {/if}
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
    width: min(820px, 100%);
    max-height: 100%;
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
    display: flex;
    min-height: 0;
  }
  .body.single {
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
    padding: 24px;
  }
  .preview {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 20px;
    background: var(--canvas);
  }
  .preview img {
    max-height: 520px;
    max-width: 100%;
    box-shadow: var(--shadow);
    background: #fff;
  }
  .ph {
    color: var(--text-3);
  }
  .side {
    width: 330px;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 18px;
    border-left: 1px solid var(--border);
  }
  .field-row {
    display: flex;
    flex-direction: column;
    gap: 6px;
    color: var(--text-2);
  }
  .insets {
    display: flex;
    flex-direction: column;
    gap: 6px;
    color: var(--text-2);
  }
  .inset-row {
    display: flex;
    gap: 12px;
    font-size: 12px;
  }
  .inset-row label {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .num {
    width: 52px;
    text-align: right;
  }
  .facts {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .facts div {
    display: flex;
    justify-content: space-between;
    color: var(--text-2);
  }
  .note {
    margin: 0;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: var(--panel-2);
    color: var(--text-2);
    font-size: 12px;
    line-height: 1.45;
  }
  .warn {
    margin: 0;
    color: var(--warn);
    font-size: 12px;
  }
  .actions {
    display: flex;
    gap: 8px;
    margin-top: auto;
  }
  .grow {
    flex: 1;
    justify-content: center;
  }
  .hint {
    margin: 0;
    color: var(--text-3);
    font-size: 12px;
  }
</style>
