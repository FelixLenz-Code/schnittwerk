<script lang="ts">
  import { editor } from "../editor.svelte";
  import Icon from "./Icon.svelte";
  import LayersPanel from "./LayersPanel.svelte";
  import PropertiesPanel from "./PropertiesPanel.svelte";
  import TracePanel from "./TracePanel.svelte";
  import StickerPanel from "./StickerPanel.svelte";

  const tabs = [
    { id: "object", label: "Objekt", icon: "sliders" },
    { id: "trace", label: "Tracen", icon: "wand" },
    { id: "sticker", label: "Sticker", icon: "sparkle" },
    { id: "layers", label: "Ebenen", icon: "layers" },
  ] as const;
</script>

<aside>
  <nav>
    {#each tabs as t}
      <button class:on={editor.rightTab === t.id} onclick={() => (editor.rightTab = t.id)}>
        <Icon name={t.icon} size={14} />
        {t.label}
        {#if t.id === "layers" && editor.objects.length}
          <span class="count">{editor.objects.length}</span>
        {/if}
      </button>
    {/each}
  </nav>
  <div class="body">
    {#if editor.rightTab === "object"}
      <PropertiesPanel />
    {:else if editor.rightTab === "trace"}
      <TracePanel />
    {:else if editor.rightTab === "sticker"}
      <StickerPanel />
    {:else}
      <LayersPanel />
    {/if}
  </div>
</aside>

<style>
  aside {
    width: 320px;
    flex: none;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border-left: 1px solid var(--border);
    min-height: 0;
  }
  nav {
    display: flex;
    gap: 2px;
    padding: 8px 8px 0;
    border-bottom: 1px solid var(--border);
  }
  nav button {
    flex: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    height: 34px;
    font-size: 12.5px;
    border: 0;
    border-bottom: 2px solid transparent;
    background: transparent;
    color: var(--text-2);
    cursor: pointer;
    font-weight: 500;
  }
  nav button:hover {
    color: var(--text);
  }
  nav button.on {
    color: var(--accent);
    border-bottom-color: var(--accent);
  }
  .count {
    font-size: 10px;
    padding: 0 5px;
    border-radius: 999px;
    background: var(--panel-2);
    color: var(--text-2);
  }
  .body {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
  }
</style>
