<script lang="ts">
  import { editor } from "../editor.svelte";
  import Icon from "./Icon.svelte";

  const items = $derived([...editor.objects].reverse());

  function select(id: string, e: MouseEvent) {
    if (e.shiftKey || e.ctrlKey) {
      editor.selection = editor.selection.includes(id)
        ? editor.selection.filter((s) => s !== id)
        : [...editor.selection, id];
    } else {
      editor.selection = [id];
    }
  }
</script>

{#if !items.length}
  <div class="empty muted">Noch keine Objekte.</div>
{:else}
  <ul>
    {#each items as o (o.id)}
      <li class:sel={editor.selection.includes(o.id)} class:hidden={!o.visible}>
        <button class="main" onclick={(e) => select(o.id, e)}>
          {#if o.type === "image"}
            <img src={editor.images[o.imageId]?.src} alt="" />
          {:else}
            <span class="chip" style:border-color={o.color}><Icon name="scissors" size={12} /></span>
          {/if}
          <span class="label">{o.name}</span>
        </button>
        <button
          class="btn icon"
          title={o.visible ? "Ausblenden" : "Einblenden"}
          onclick={() => {
            editor.checkpoint();
            o.visible = !o.visible;
            if (!o.visible) editor.selection = editor.selection.filter((s) => s !== o.id);
          }}><Icon name={o.visible ? "eye" : "eyeOff"} size={14} /></button
        >
        <button
          class="btn icon"
          class:active={o.locked}
          title={o.locked ? "Entsperren" : "Sperren"}
          onclick={() => {
            editor.checkpoint();
            o.locked = !o.locked;
          }}><Icon name={o.locked ? "lock" : "unlock"} size={14} /></button
        >
      </li>
    {/each}
  </ul>
{/if}

<style>
  ul {
    list-style: none;
    margin: 0;
    padding: 8px;
  }
  li {
    display: flex;
    align-items: center;
    gap: 2px;
    border-radius: var(--radius-sm);
    padding-right: 4px;
  }
  li:hover {
    background: var(--panel-2);
  }
  li.sel {
    background: var(--accent-soft);
  }
  li.hidden .label,
  li.hidden img {
    opacity: 0.45;
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 8px;
    border: 0;
    background: transparent;
    cursor: pointer;
    text-align: left;
  }
  img {
    width: 22px;
    height: 22px;
    object-fit: cover;
    border-radius: 4px;
    border: 1px solid var(--border);
  }
  .chip {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border-radius: 4px;
    border: 2px solid;
    color: var(--text-2);
  }
  .label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .btn.icon {
    width: 26px;
    height: 26px;
    color: var(--text-3);
  }
  .btn.icon.active {
    color: var(--accent);
  }
  .empty {
    padding: 28px 20px;
    color: var(--text-3);
  }
</style>
