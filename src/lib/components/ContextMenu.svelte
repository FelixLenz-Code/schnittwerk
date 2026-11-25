<script lang="ts" module>
  // Right-click menu of the work area.
  export const contextMenu = $state<{ open: boolean; x: number; y: number; onObject: boolean }>({ open: false, x: 0, y: 0, onObject: false });
</script>

<script lang="ts">
  import { CUT_TYPES, flip, putTextOnPath, selectedPaths, setCutColor, textOnPathPair } from "../actions";
  import { editor } from "../editor.svelte";
  import { t } from "../i18n.svelte";
  import { copySelection, paste } from "../io.svelte";
  import { nodes } from "../nodes.svelte";
  import type { PathObj } from "../types";
  import Icon from "./Icon.svelte";

  let menu = $state<HTMLDivElement>();

  function close() {
    contextMenu.open = false;
  }

  function run(fn: () => void) {
    close();
    fn();
  }

  const paths = $derived(selectedPaths());
  const single = $derived(editor.selected.length === 1 ? editor.selected[0] : null);
  const grouped = $derived(editor.selected.some((o) => o.group));
  const pair = $derived(textOnPathPair());

  /** Keep the menu inside the window. */
  const pos = $derived.by(() => {
    const w = menu?.offsetWidth ?? 220, h = menu?.offsetHeight ?? 360;
    return { x: Math.min(contextMenu.x, window.innerWidth - w - 8), y: Math.min(contextMenu.y, window.innerHeight - h - 8) };
  });

  function clearGuides() {
    editor.guides = { v: [], h: [] };
    editor.dirty = true;
    editor.rev++;
  }
</script>

<svelte:window
  onpointerdown={(e) => contextMenu.open && !menu?.contains(e.target as Node) && close()}
  onkeydown={() => contextMenu.open && close()}
  onwheel={() => contextMenu.open && close()}
  onblur={close}
/>

{#if contextMenu.open}
  <div class="menu" bind:this={menu} style:left="{pos.x}px" style:top="{pos.y}px" role="menu">
    {#if contextMenu.onObject && editor.selected.length}
      <button role="menuitem" onclick={() => run(() => copySelection(true))}><Icon name="scissors" size={14} /> {t("Ausschneiden")}<kbd>{t("Strg")}+X</kbd></button>
      <button role="menuitem" onclick={() => run(() => copySelection())}><Icon name="copy" size={14} /> {t("Kopieren")}<kbd>{t("Strg")}+C</kbd></button>
      <button role="menuitem" onclick={() => run(() => paste())}><Icon name="download" size={14} /> {t("Einfügen")}<kbd>{t("Strg")}+V</kbd></button>
      <button role="menuitem" onclick={() => run(() => editor.duplicateSelected())}><Icon name="layers" size={14} /> {t("Duplizieren")}<kbd>{t("Strg")}+D</kbd></button>
      <button role="menuitem" class="danger" onclick={() => run(() => editor.removeSelected())}><Icon name="trash" size={14} /> {t("Löschen")}<kbd>{t("Entf")}</kbd></button>
      <div class="sep"></div>
      {#if grouped}
        <button role="menuitem" onclick={() => run(() => editor.ungroupSelected())}><Icon name="layers" size={14} /> {t("Gruppierung lösen")}<kbd>{t("Strg")}+⇧+G</kbd></button>
      {:else if editor.selected.length > 1}
        <button role="menuitem" onclick={() => run(() => editor.groupSelected())}><Icon name="layers" size={14} /> {t("Gruppieren")}<kbd>{t("Strg")}+G</kbd></button>
      {/if}
      <button role="menuitem" onclick={() => run(() => editor.restack(1))}><Icon name="arrowUp" size={14} /> {t("Nach vorne")}<kbd>{t("Strg")}+↑</kbd></button>
      <button role="menuitem" onclick={() => run(() => editor.restack(-1))}><Icon name="arrowDown" size={14} /> {t("Nach hinten")}<kbd>{t("Strg")}+↓</kbd></button>
      <button role="menuitem" onclick={() => run(() => flip("x"))}><Icon name="flipH" size={14} /> {t("Spiegeln")}</button>
      <button role="menuitem" onclick={() => run(() => flip("y"))}><Icon name="flipV" size={14} /> {t("Kippen")}</button>
      {#if paths.length}
        <div class="sep"></div>
        <div class="label">{t("Schnittart")}</div>
        <div class="cuts">
          {#each CUT_TYPES as ct}
            <button
              role="menuitem"
              class:on={paths.every((o) => o.color.toLowerCase() === ct.color)}
              title={t(ct.hint)}
              onclick={() => run(() => setCutColor(ct.color))}><span class="dot" style:background={ct.color}></span>{t(ct.label)}</button
            >
          {/each}
        </div>
      {/if}
      <div class="sep"></div>
      {#if single?.type === "path" && !single.locked}
        <button role="menuitem" onclick={() => run(() => nodes.start(single as PathObj))}><Icon name="pen" size={14} /> {t("Knoten bearbeiten")}</button>
      {/if}
      {#if pair}
        <button role="menuitem" onclick={() => run(putTextOnPath)}><Icon name="type" size={14} /> {t("Text auf Pfad")}</button>
      {/if}
      <button role="menuitem" onclick={() => run(() => editor.fitSelection())}><Icon name="fit" size={14} /> {t("Auswahl einpassen")}<kbd>{t("Strg")}+2</kbd></button>
    {:else}
      <button role="menuitem" onclick={() => run(() => paste())}><Icon name="download" size={14} /> {t("Einfügen")}<kbd>{t("Strg")}+V</kbd></button>
      <button role="menuitem" onclick={() => run(() => editor.selectAll())}><Icon name="pointer" size={14} /> {t("Alles auswählen")}<kbd>{t("Strg")}+A</kbd></button>
      <div class="sep"></div>
      <button role="menuitem" onclick={() => run(() => editor.fitMat())}><Icon name="fit" size={14} /> {t("Matte einpassen")}<kbd>{t("Strg")}+0</kbd></button>
      <button role="menuitem" onclick={() => run(() => editor.fitSelection())}><Icon name="zoomIn" size={14} /> {t("Alles einpassen")}<kbd>{t("Strg")}+2</kbd></button>
      <button role="menuitem" onclick={() => run(() => (editor.showGrid = !editor.showGrid))}><Icon name="grid" size={14} /> {editor.showGrid ? t("Raster ausblenden") : t("Raster anzeigen")}</button>
      {#if editor.guides.v.length || editor.guides.h.length}
        <button role="menuitem" onclick={() => run(clearGuides)}><Icon name="x" size={14} /> {t("Alle Hilfslinien entfernen")}</button>
      {/if}
    {/if}
  </div>
{/if}

<style>
  .menu {
    position: fixed;
    z-index: 80;
    min-width: 220px;
    padding: 5px;
    border-radius: var(--radius);
    background: var(--panel);
    border: 1px solid var(--border);
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.25);
    display: flex;
    flex-direction: column;
  }
  button {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 28px;
    padding: 0 8px;
    border: 0;
    border-radius: var(--radius-sm);
    background: none;
    text-align: left;
    cursor: pointer;
  }
  button:hover {
    background: var(--panel-2);
  }
  button.danger:hover {
    color: var(--danger);
  }
  kbd {
    margin-left: auto;
    padding-left: 16px;
    font: inherit;
    font-size: 11px;
    color: var(--text-3);
  }
  .sep {
    height: 1px;
    margin: 4px 2px;
    background: var(--border);
  }
  .label {
    padding: 2px 8px;
    color: var(--text-3);
    font-size: 11px;
  }
  .cuts {
    display: flex;
    gap: 2px;
  }
  .cuts button {
    flex: 1;
    gap: 5px;
    font-size: 12px;
    padding: 0 6px;
  }
  .cuts button.on {
    background: var(--accent-soft);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex: none;
  }
</style>
