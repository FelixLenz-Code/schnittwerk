<script lang="ts">
  // Material of the project, next to the cut button: always visible and
  // changeable without opening the cut dialog.
  import { editor } from "../editor.svelte";
  import { t } from "../i18n.svelte";
  import { lastMaterialId, loadCustom, PRESETS, rememberMaterial, type Material } from "../materials";
  import { ui } from "../ui.svelte";
  import Icon from "./Icon.svelte";

  let open = $state(false);
  let root = $state<HTMLElement>();
  let custom = $state<Material[]>(loadCustom());

  const currentId = $derived(editor.material ?? lastMaterialId());
  const current = $derived([...PRESETS, ...custom].find((m) => m.id === currentId) ?? PRESETS[0]);

  function toggle() {
    // Own materials may have changed in the settings or the cut dialog.
    if (!open) custom = loadCustom();
    open = !open;
  }

  function pick(id: string) {
    if (id !== editor.material) {
      editor.material = id;
      editor.dirty = true;
    }
    rememberMaterial(id);
    open = false;
  }
</script>

<svelte:window
  onpointerdown={(e) => open && root && !root.contains(e.target as Node) && (open = false)}
  onkeydown={(e) => open && e.key === "Escape" && (open = false)}
/>

<div class="wrap" bind:this={root} data-tour="material">
  <button class="chip" class:open onclick={toggle} title={t("Material für diesen Auftrag – wird beim Schneiden vorgewählt")} aria-haspopup="menu" aria-expanded={open}>
    <Icon name={current.tool === "pen" ? "pen" : "vinyl"} size={14} />
    <span class="name">{current.custom ? current.name : t(current.name)}</span>
    <span class="chev"><Icon name="chevronLeft" size={11} /></span>
  </button>
  {#if open}
    <div class="pop" role="menu">
      <div class="group-title">{t("Vorlagen")}</div>
      {#each PRESETS as m}
        <button class="item" class:on={m.id === currentId} role="menuitemradio" aria-checked={m.id === currentId} onclick={() => pick(m.id)}>
          <span class="radio"></span>{t(m.name)}
        </button>
      {/each}
      {#if custom.length}
        <div class="group-title">{t("Eigene")}</div>
        {#each custom as m}
          <button class="item" class:on={m.id === currentId} role="menuitemradio" aria-checked={m.id === currentId} onclick={() => pick(m.id)}>
            <span class="radio"></span>{m.name}
          </button>
        {/each}
      {/if}
      <div class="sep"></div>
      <button class="item manage" onclick={() => { open = false; ui.materialsOpen = true; }}>
        <Icon name="sliders" size={13} /> {t("Eigene Materialien verwalten …")}
      </button>
    </div>
  {/if}
</div>

<style>
  .wrap {
    position: relative;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    max-width: 210px;
    padding: 0 9px 0 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--panel);
    color: var(--text);
    font-size: 12.5px;
    cursor: pointer;
  }
  .chip:hover,
  .chip.open {
    border-color: var(--border-strong);
  }
  .chip :global(svg) {
    flex: none;
    color: var(--text-2);
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chev {
    display: inline-flex;
    transform: rotate(-90deg);
  }
  .pop {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    z-index: 40;
    width: 250px;
    max-height: 70vh;
    overflow-y: auto;
    padding: 6px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--panel);
    box-shadow: var(--shadow);
  }
  .group-title {
    padding: 8px 8px 4px;
    color: var(--text-3);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100%;
    min-height: 30px;
    padding: 4px 8px;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .item:hover {
    background: var(--panel-2);
  }
  .item.on {
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
  }
  .radio {
    width: 12px;
    height: 12px;
    flex: none;
    border: 1.5px solid var(--border-strong);
    border-radius: 50%;
  }
  .item.on .radio {
    border: 4px solid var(--accent);
  }
  .sep {
    height: 1px;
    margin: 6px 4px;
    background: var(--border);
  }
  .manage {
    color: var(--text-2);
  }
</style>
