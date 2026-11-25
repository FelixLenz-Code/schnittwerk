<script lang="ts">
  import { TEMPLATES, newFromTemplate, templatesUi } from "../templates.svelte";
  import Icon from "./Icon.svelte";
  import { t } from "../i18n.svelte";

  /** Thumbnail shapes (viewBox 0 0 40 40). */
  const THUMBS: Record<string, string> = {
    empty: "M8 4H32V36H8Z",
    "stickers-round": "M7 13a6 6 0 1 0 12 0a6 6 0 1 0 -12 0M21 13a6 6 0 1 0 12 0a6 6 0 1 0 -12 0M7 27a6 6 0 1 0 12 0a6 6 0 1 0 -12 0M21 27a6 6 0 1 0 12 0a6 6 0 1 0 -12 0",
    labels: "M6 7h12v7H6ZM22 7h12v7H22ZM6 17h12v7H6ZM22 17h12v7H22ZM6 27h12v7H6ZM22 27h12v7H22Z",
    "labels-round": "M6 10a4 4 0 1 0 8 0a4 4 0 1 0 -8 0M16 10a4 4 0 1 0 8 0a4 4 0 1 0 -8 0M26 10a4 4 0 1 0 8 0a4 4 0 1 0 -8 0M6 20a4 4 0 1 0 8 0a4 4 0 1 0 -8 0M16 20a4 4 0 1 0 8 0a4 4 0 1 0 -8 0M26 20a4 4 0 1 0 8 0a4 4 0 1 0 -8 0M6 30a4 4 0 1 0 8 0a4 4 0 1 0 -8 0M16 30a4 4 0 1 0 8 0a4 4 0 1 0 -8 0M26 30a4 4 0 1 0 8 0a4 4 0 1 0 -8 0",
    card: "M10 4H30V36H10Z",
    bookmarks: "M8 4h7v32H8ZM17 4h7v32h-7ZM26 4h7v32h-7Z",
    tags: "M9 8l3-3h4l3 3v12H9ZM21 8l3-3h4l3 3v12H21ZM9 24l3-3h4l3 3v12H9ZM21 24l3-3h4l3 3v12H21Z",
    "sticker-sheet-frame": "M7 5H33V35H7Z",
  };

  function close() {
    templatesUi.open = false;
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

<div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && close()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label={t("Neu aus Vorlage")}>
    <header>
      <h2><Icon name="filePlus" size={18} /> {t("Neu aus Vorlage")}</h2>
      <button class="btn icon" onclick={close} aria-label={t("Schließen")}><Icon name="x" /></button>
    </header>
    <div class="grid">
      {#each TEMPLATES as tpl}
        <button class="card" onclick={() => newFromTemplate(tpl)}>
          <svg viewBox="0 0 40 40" aria-hidden="true">
            <path d={THUMBS[tpl.id] ?? THUMBS.empty} fill="none" stroke="var(--cut)" stroke-width="1.2" />
            {#if tpl.id === "card"}<line x1="10" y1="20" x2="30" y2="20" stroke="var(--text-3)" stroke-dasharray="2 2" />{/if}
          </svg>
          <span class="name">{t(tpl.name)}</span>
          <span class="hint">{t(tpl.hint)}</span>
          {#if tpl.printCut}<span class="badge">Print & Cut</span>{/if}
        </button>
      {/each}
    </div>
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
    width: min(860px, 100%);
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
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
    gap: 12px;
    padding: 16px;
    overflow-y: auto;
  }
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    padding: 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--panel-2);
    text-align: left;
    cursor: pointer;
  }
  .card:hover {
    border-color: var(--accent);
  }
  svg {
    width: 64px;
    height: 64px;
    margin-bottom: 4px;
    background: var(--mat);
    border-radius: 6px;
  }
  .name {
    font-weight: 600;
  }
  .hint {
    color: var(--text-3);
    font-size: 12px;
    line-height: 1.4;
  }
  .badge {
    position: absolute;
    top: 10px;
    right: 10px;
    padding: 1px 6px;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 11px;
  }
</style>
