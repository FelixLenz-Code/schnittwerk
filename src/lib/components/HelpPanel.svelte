<script lang="ts">
  // Help tab: searchable built-in documentation.
  import { DOCS_DE, DOCS_EN, renderDoc, type DocSection } from "../docs";
  import { DOCS_FR } from "../fr";
  import { DOCS_ES } from "../es";
  import { DOCS_NL } from "../nl";
  import { i18n, t } from "../i18n.svelte";
  import { startTour } from "../tour.svelte";
  import { ui } from "../ui.svelte";
  import { tick } from "svelte";
  import { api } from "../api";
  import Icon from "./Icon.svelte";

  let query = $state("");
  let openId = $state<string | null>("start");

  // The menu bar can ask for a chapter (e.g. keyboard shortcuts).
  $effect(() => {
    const id = ui.helpSection;
    if (!id) return;
    query = "";
    openId = id;
    ui.helpSection = null;
    tick().then(() => document.getElementById(`help-${id}`)?.scrollIntoView({ block: "start" }));
  });

  const DOCS: Record<string, DocSection[]> = { de: DOCS_DE, en: DOCS_EN, fr: DOCS_FR, es: DOCS_ES, nl: DOCS_NL };
  const docs = $derived(DOCS[i18n.lang] ?? DOCS_EN);
  const hits = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return docs;
    return docs.filter((d) => d.heading.toLowerCase().includes(q) || d.text.toLowerCase().includes(q));
  });
</script>

<div class="help">
  <div class="top">
    <div class="search">
      <Icon name="search" size={14} />
      <input class="field" placeholder={t("Hilfe durchsuchen …")} bind:value={query} />
    </div>
    <div class="row">
      <button class="btn outline" onclick={startTour}><Icon name="play" size={14} /> {t("Einführung starten")}</button>
      <button class="btn" onclick={() => api.openUrl("https://github.com/FelixLenz-Code/schnittwerk/issues")} title={t("Fehler melden oder Frage stellen")}>GitHub</button>
    </div>
  </div>
  {#each hits as d (d.id)}
    {@const open = !!query.trim() || openId === d.id}
    <section id="help-{d.id}">
      <button class="head" onclick={() => (openId = openId === d.id ? null : d.id)} aria-expanded={open}>
        <span class="chev" class:open><Icon name="chevronLeft" size={12} /></span>
        {d.heading}
      </button>
      {#if open}
        <div class="doc">{@html renderDoc(d.text)}</div>
      {/if}
    </section>
  {:else}
    <p class="muted none">{t("Nichts gefunden.")}</p>
  {/each}
</div>

<style>
  .top {
    padding: 12px 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    border-bottom: 1px solid var(--border);
  }
  .search {
    position: relative;
    display: flex;
    align-items: center;
    color: var(--text-3);
  }
  .search :global(svg) {
    position: absolute;
    left: 9px;
  }
  .search input {
    width: 100%;
    padding-left: 28px;
  }
  .row {
    display: flex;
    gap: 6px;
  }
  section {
    border-bottom: 1px solid var(--border);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 11px 16px;
    border: 0;
    background: none;
    cursor: pointer;
    text-align: left;
    font-weight: 600;
  }
  .head:hover {
    background: var(--panel-2);
  }
  .chev {
    display: inline-flex;
    color: var(--text-3);
    transform: rotate(180deg);
    transition: transform 0.12s;
  }
  .chev.open {
    transform: rotate(-90deg);
  }
  .doc {
    padding: 0 16px 12px;
    color: var(--text-2);
    line-height: 1.5;
    font-size: 12.5px;
  }
  .doc :global(p) {
    margin: 0 0 8px;
  }
  .doc :global(ul) {
    margin: 0 0 8px;
    padding-left: 18px;
  }
  .doc :global(li) {
    margin-bottom: 3px;
  }
  .doc :global(strong) {
    color: var(--text);
  }
  .doc :global(kbd) {
    font: inherit;
    font-size: 11px;
    padding: 0 4px;
    border-radius: 4px;
    border: 1px solid var(--border);
    background: var(--panel-2);
  }
  .none {
    padding: 16px;
  }
</style>
