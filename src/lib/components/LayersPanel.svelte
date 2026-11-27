<script lang="ts">
  // Layer list: select, rename (double-click), reorder (drag), hide, lock,
  // and the usual object actions in a small toolbar or the right-click menu.
  import { editor } from "../editor.svelte";
  import Icon from "./Icon.svelte";
  import { t } from "../i18n.svelte";
  import { contextMenu } from "./ContextMenu.svelte";

  /** Front-most object first. */
  const items = $derived([...editor.objects].reverse());
  const hasSel = $derived(editor.selection.length > 0);
  const grouped = $derived(editor.selected.some((o) => o.group));

  function select(id: string, e: MouseEvent) {
    if (dragJustEnded) return;
    if (e.shiftKey || e.ctrlKey) {
      editor.selection = editor.selection.includes(id) ? editor.selection.filter((s) => s !== id) : [...editor.selection, id];
    } else {
      editor.selection = [id];
    }
  }

  // ---------- rename ----------
  let editing = $state<string | null>(null);
  function startRename(id: string) {
    editing = id;
  }
  function commitRename(id: string, value: string) {
    const o = editor.byId(id);
    const name = value.trim();
    editing = null;
    if (!o || !name || name === o.name) return;
    editor.checkpoint("Umbenennen");
    o.name = name;
  }
  function focusSelect(el: HTMLInputElement) {
    el.focus();
    el.select();
  }

  // ---------- right-click ----------
  function openMenu(id: string, e: MouseEvent) {
    e.preventDefault();
    if (!editor.selection.includes(id)) editor.selection = [id];
    Object.assign(contextMenu, { open: true, x: e.clientX, y: e.clientY, onObject: true });
  }

  // ---------- drag to reorder ----------
  let list = $state<HTMLUListElement>();
  let drag = $state<{ id: string; startY: number; active: boolean; target: number | null } | null>(null);
  let dragJustEnded = false;

  function onDown(id: string, e: PointerEvent) {
    if (e.button !== 0 || editing) return;
    drag = { id, startY: e.clientY, active: false, target: null };
  }
  function onMove(e: PointerEvent) {
    if (!drag || !list) return;
    if (!drag.active) {
      if (Math.abs(e.clientY - drag.startY) < 5) return;
      drag.active = true;
      list.setPointerCapture(e.pointerId);
    }
    // Drop position = gap between rows (0 = above the first row).
    const rows = [...list.querySelectorAll("li")];
    let gap = rows.length;
    for (let i = 0; i < rows.length; i++) {
      const r = rows[i].getBoundingClientRect();
      if (e.clientY < r.top + r.height / 2) {
        gap = i;
        break;
      }
    }
    drag.target = gap;
  }
  function onUp(e: PointerEvent) {
    if (!drag) return;
    const d = drag;
    drag = null;
    if (!d.active || d.target === null) return;
    if (list?.hasPointerCapture(e.pointerId)) list.releasePointerCapture(e.pointerId);
    dragJustEnded = true;
    setTimeout(() => (dragJustEnded = false), 0);
    const from = items.findIndex((o) => o.id === d.id);
    let to = d.target;
    if (from < 0 || to === from || to === from + 1) return;
    if (to > from) to--;
    // Reorder in list order (front first), then store back to front.
    const order = [...items];
    const [moved] = order.splice(from, 1);
    order.splice(to, 0, moved);
    editor.checkpoint("Anordnen");
    editor.objects = order.reverse();
  }
</script>

{#if !items.length}
  <div class="empty muted">{t("Noch keine Objekte.")}</div>
{:else}
  <div class="toolbar">
    <button class="btn icon" disabled={!hasSel} title={t("Duplizieren")} aria-label={t("Duplizieren")} onclick={() => editor.duplicateSelected()}><Icon name="copy" size={14} /></button>
    <button class="btn icon" disabled={!hasSel} title={t("Nach vorne (Strg+↑)")} aria-label={t("Nach vorne")} onclick={() => editor.restack(1)}><Icon name="arrowUp" size={14} /></button>
    <button class="btn icon" disabled={!hasSel} title={t("Nach hinten (Strg+↓)")} aria-label={t("Nach hinten")} onclick={() => editor.restack(-1)}><Icon name="arrowDown" size={14} /></button>
    {#if grouped}
      <button class="btn icon" title={t("Gruppierung aufheben (Strg+Shift+G)")} aria-label={t("Gruppierung lösen")} onclick={() => editor.ungroupSelected()}><Icon name="layers" size={14} /></button>
    {:else}
      <button class="btn icon" disabled={editor.selection.length < 2} title={t("Gruppieren (Strg+G)")} aria-label={t("Gruppieren")} onclick={() => editor.groupSelected()}><Icon name="layers" size={14} /></button>
    {/if}
    <span class="grow"></span>
    <button class="btn icon danger" disabled={!hasSel} title={t("Löschen")} aria-label={t("Löschen")} onclick={() => editor.removeSelected()}><Icon name="trash" size={14} /></button>
  </div>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <ul bind:this={list} class:dragging={drag?.active} onpointermove={onMove} onpointerup={onUp} onpointercancel={() => (drag = null)}>
    {#each items as o, i (o.id)}
      <li
        class:sel={editor.selection.includes(o.id)}
        class:hidden={!o.visible}
        class:moving={drag?.active && drag.id === o.id}
        class:drop-before={drag?.active && drag.target === i}
        class:drop-after={drag?.active && drag.target === items.length && i === items.length - 1}
        class:grouped={!!o.group}
        oncontextmenu={(e) => openMenu(o.id, e)}
      >
        {#if editing === o.id}
          <span class="main">
            {#if o.type === "image"}
              <img src={editor.images[o.imageId]?.src} alt="" />
            {:else}
              <span class="chip" style:border-color={o.color}><Icon name={o.text ? "type" : "scissors"} size={12} /></span>
            {/if}
            <input
              class="rename"
              value={o.name}
              use:focusSelect
              onkeydown={(e) => {
                e.stopPropagation();
                if (e.key === "Enter") commitRename(o.id, e.currentTarget.value);
                else if (e.key === "Escape") editing = null;
              }}
              onblur={(e) => commitRename(o.id, e.currentTarget.value)}
            />
          </span>
        {:else}
          <button
            class="main"
            title={t("Doppelklick: umbenennen · Ziehen: Reihenfolge ändern · Rechtsklick: weitere Aktionen")}
            onpointerdown={(e) => onDown(o.id, e)}
            onclick={(e) => select(o.id, e)}
            ondblclick={() => startRename(o.id)}
          >
            {#if o.type === "image"}
              <img src={editor.images[o.imageId]?.src} alt="" />
            {:else}
              <span class="chip" style:border-color={o.color}><Icon name={o.text ? "type" : "scissors"} size={12} /></span>
            {/if}
            <span class="label">{t(o.name)}</span>
          </button>
        {/if}
        <button
          class="btn icon"
          title={o.visible ? t("Ausblenden") : t("Einblenden")}
          onclick={() => {
            editor.checkpoint("Ein-/Ausblenden");
            o.visible = !o.visible;
            if (!o.visible) editor.selection = editor.selection.filter((s) => s !== o.id);
          }}><Icon name={o.visible ? "eye" : "eyeOff"} size={14} /></button
        >
        <button
          class="btn icon"
          class:active={o.locked}
          title={o.locked ? t("Entsperren") : t("Sperren")}
          onclick={() => {
            editor.checkpoint("Sperren");
            o.locked = !o.locked;
          }}><Icon name={o.locked ? "lock" : "unlock"} size={14} /></button
        >
      </li>
    {/each}
  </ul>
{/if}

<style>
  .toolbar {
    position: sticky;
    top: 0;
    z-index: 1;
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 6px 8px;
    border-bottom: 1px solid var(--border);
    background: var(--panel);
  }
  .toolbar .grow {
    flex: 1;
  }
  .toolbar .btn.danger:hover:not(:disabled) {
    color: var(--danger);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 8px;
  }
  ul.dragging {
    cursor: grabbing;
    user-select: none;
  }
  li {
    position: relative;
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
  li.grouped::before {
    content: "";
    position: absolute;
    left: 2px;
    top: 8px;
    bottom: 8px;
    width: 2px;
    border-radius: 2px;
    background: var(--border-strong);
  }
  li.moving {
    opacity: 0.4;
  }
  li.drop-before::after,
  li.drop-after::after {
    content: "";
    position: absolute;
    left: 6px;
    right: 6px;
    height: 2px;
    border-radius: 2px;
    background: var(--accent);
  }
  li.drop-before::after {
    top: -1px;
  }
  li.drop-after::after {
    bottom: -1px;
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
  .rename {
    flex: 1;
    min-width: 0;
    height: 24px;
    padding: 0 6px;
    border: 1px solid var(--accent);
    border-radius: 5px;
    background: var(--panel);
    color: var(--text);
    font: inherit;
    outline: none;
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
    flex: none;
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
