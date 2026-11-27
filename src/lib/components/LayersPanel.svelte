<script lang="ts">
  // Layer list: select, rename (double-click), reorder (drag), hide, lock,
  // groups as collapsible entries, and the usual object actions in a small
  // toolbar or the right-click menu.
  import { editor } from "../editor.svelte";
  import Icon from "./Icon.svelte";
  import { t } from "../i18n.svelte";
  import { contextMenu } from "./ContextMenu.svelte";
  import type { Obj } from "../types";

  /** Front-most object first. */
  const items = $derived([...editor.objects].reverse());
  const hasSel = $derived(editor.selection.length > 0);
  const grouped = $derived(editor.selected.some((o) => o.group));

  // ---------- units and rows ----------
  /** A group (all its members) or a single object, front-most first. */
  type Unit = { key: string; group: string | null; objs: Obj[] };
  const units = $derived.by((): Unit[] => {
    const out: Unit[] = [];
    const byGroup = new Map<string, Unit>();
    for (const o of items) {
      if (!o.group) {
        out.push({ key: o.id, group: null, objs: [o] });
        continue;
      }
      let u = byGroup.get(o.group);
      if (!u) {
        u = { key: "g:" + o.group, group: o.group, objs: [] };
        byGroup.set(o.group, u);
        out.push(u);
      }
      u.objs.push(o);
    }
    return out;
  });

  /** Expanded groups (collapsed by default – stickers are three objects each). */
  let expanded = $state<Record<string, boolean>>({});
  type Row = { kind: "group"; unit: number; u: Unit } | { kind: "item"; unit: number; o: Obj; child: boolean };
  const rows = $derived.by((): Row[] => {
    const out: Row[] = [];
    units.forEach((u, i) => {
      if (!u.group) out.push({ kind: "item", unit: i, o: u.objs[0], child: false });
      else {
        out.push({ kind: "group", unit: i, u });
        if (expanded[u.group]) for (const o of u.objs) out.push({ kind: "item", unit: i, o, child: true });
      }
    });
    return out;
  });

  const groupName = (u: Unit) => {
    const img = u.objs.find((o) => o.type === "image");
    return t((img ?? u.objs[u.objs.length - 1]).name);
  };
  const groupSel = (u: Unit) => {
    const n = u.objs.filter((o) => editor.selection.includes(o.id)).length;
    return n === 0 ? "" : n === u.objs.length ? "sel" : "part";
  };

  function select(id: string, e: MouseEvent) {
    if (dragJustEnded) return;
    if (e.shiftKey || e.ctrlKey) {
      editor.selection = editor.selection.includes(id) ? editor.selection.filter((s) => s !== id) : [...editor.selection, id];
    } else {
      editor.selection = [id];
    }
  }
  function selectGroup(u: Unit, e: MouseEvent) {
    if (dragJustEnded) return;
    const ids = u.objs.map((o) => o.id);
    if (e.shiftKey || e.ctrlKey) {
      const all = ids.every((id) => editor.selection.includes(id));
      editor.selection = all ? editor.selection.filter((s) => !ids.includes(s)) : [...new Set([...editor.selection, ...ids])];
    } else {
      editor.selection = ids;
    }
  }
  function toggleGroup(g: string) {
    expanded[g] = !expanded[g];
  }
  function setAll(u: Unit, key: "visible" | "locked", value: boolean) {
    editor.checkpoint(key === "visible" ? "Ein-/Ausblenden" : "Sperren");
    for (const o of u.objs) o[key] = value;
    if (key === "visible" && !value) editor.selection = editor.selection.filter((s) => !u.objs.some((o) => o.id === s));
  }

  // ---------- mini preview of a path ----------
  function thumb(o: Obj) {
    if (o.type !== "path") return null;
    const [x, y, w, h] = o.bbox;
    const pad = Math.max(w, h) * 0.06 || 1;
    return {
      viewBox: `${x - pad} ${y - pad} ${w + 2 * pad} ${h + 2 * pad}`,
      transform: `rotate(${o.rot}deg) scale(${o.flipX ? -1 : 1}, ${o.flipY ? -1 : 1})`,
    };
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
  /** F2 renames the selected layer (like in a file manager). */
  function onKey(e: KeyboardEvent) {
    if (e.key !== "F2" || editing || editor.selection.length !== 1) return;
    const tgt = e.target as HTMLElement;
    if (tgt instanceof HTMLInputElement || tgt instanceof HTMLTextAreaElement || tgt instanceof HTMLSelectElement) return;
    e.preventDefault();
    const id = editor.selection[0];
    const o = editor.byId(id);
    if (o?.group) expanded[o.group] = true;
    startRename(id);
  }

  // ---------- right-click ----------
  function openMenu(ids: string[], e: MouseEvent) {
    e.preventDefault();
    if (!ids.every((id) => editor.selection.includes(id))) editor.selection = ids;
    Object.assign(contextMenu, { open: true, x: e.clientX, y: e.clientY, onObject: true });
  }

  // ---------- drag to reorder (whole units: a group moves as one) ----------
  let list = $state<HTMLUListElement>();
  let drag = $state<{ unit: number; startY: number; active: boolean; target: number | null } | null>(null);
  let dragJustEnded = false;

  function onDown(unit: number, e: PointerEvent) {
    if (e.button !== 0 || editing) return;
    drag = { unit, startY: e.clientY, active: false, target: null };
  }
  /** Row index where a new unit could start for a pointer at y. */
  function gapAt(y: number): number {
    const els = list ? [...list.querySelectorAll(":scope > li")] : [];
    for (let i = 0; i < els.length; i++) {
      const r = els[i].getBoundingClientRect();
      if (y < r.top + r.height / 2) {
        // Not between the members of a group: skip to the next unit start.
        let k = i;
        while (k < rows.length && rows[k].kind === "item" && (rows[k] as { child: boolean }).child) k++;
        return k;
      }
    }
    return rows.length;
  }
  function onMove(e: PointerEvent) {
    if (!drag || !list) return;
    if (!drag.active) {
      if (Math.abs(e.clientY - drag.startY) < 5) return;
      drag.active = true;
      list.setPointerCapture(e.pointerId);
    }
    drag.target = gapAt(e.clientY);
  }
  function onUp(e: PointerEvent) {
    if (!drag) return;
    const d = drag;
    drag = null;
    if (!d.active || d.target === null) return;
    if (list?.hasPointerCapture(e.pointerId)) list.releasePointerCapture(e.pointerId);
    dragJustEnded = true;
    setTimeout(() => (dragJustEnded = false), 0);
    // Unit index in front of which the dragged unit goes.
    let to = d.target < rows.length ? rows[d.target].unit : units.length;
    const from = d.unit;
    if (to === from || to === from + 1) return;
    if (to > from) to--;
    const order = [...units];
    const [moved] = order.splice(from, 1);
    order.splice(to, 0, moved);
    editor.checkpoint("Anordnen");
    editor.objects = order.flatMap((u) => u.objs).reverse();
  }
</script>

<svelte:window onkeydown={onKey} />

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
  {#snippet icon(o: Obj)}
    {#if o.type === "image"}
      <img src={editor.images[o.imageId]?.src} alt="" />
    {:else}
      {@const th = thumb(o)}
      <span class="thumb" style:--c={o.color}>
        {#if th}
          <svg viewBox={th.viewBox} style:transform={th.transform} preserveAspectRatio="xMidYMid meet" aria-hidden="true">
            <path d={o.d} fill={o.fill || o.text ? o.color : "none"} fill-opacity={o.text ? 1 : 0.18} stroke={o.color} stroke-width="1.4" vector-effect="non-scaling-stroke" />
          </svg>
        {/if}
      </span>
    {/if}
  {/snippet}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <ul bind:this={list} class:dragging={drag?.active} onpointermove={onMove} onpointerup={onUp} onpointercancel={() => (drag = null)}>
    {#each rows as row, i (row.kind === "group" ? row.u.key : row.o.id)}
      {@const dropBefore = drag?.active && drag.target === i}
      {@const dropAfter = drag?.active && drag.target === rows.length && i === rows.length - 1}
      {@const moving = drag?.active && drag.unit === row.unit}
      {#if row.kind === "group"}
        {@const u = row.u}
        {@const g = u.group ?? ""}
        {@const allHidden = u.objs.every((o) => !o.visible)}
        {@const allLocked = u.objs.every((o) => o.locked)}
        <li class="group {groupSel(u)}" class:hidden={allHidden} class:moving class:drop-before={dropBefore} class:drop-after={dropAfter} oncontextmenu={(e) => openMenu(u.objs.map((o) => o.id), e)}>
          <button class="btn icon fold" class:open={expanded[g]} title={expanded[g] ? t("Zuklappen") : t("Aufklappen")} aria-label={expanded[g] ? t("Zuklappen") : t("Aufklappen")} aria-expanded={!!expanded[g]} onclick={() => toggleGroup(g)}>
            <Icon name="chevronLeft" size={12} />
          </button>
          <button class="main" title={t("Gruppe – Doppelklick klappt auf · Ziehen: Reihenfolge ändern · Rechtsklick: weitere Aktionen")} onpointerdown={(e) => onDown(row.unit, e)} onclick={(e) => selectGroup(u, e)} ondblclick={() => toggleGroup(g)}>
            {@render icon(u.objs.find((o) => o.type === "image") ?? u.objs[0])}
            <span class="label">{groupName(u)}</span>
            <span class="count" title={t("{n} Objekte in der Gruppe", { n: u.objs.length })}>{u.objs.length}</span>
          </button>
          <button class="btn icon" title={allHidden ? t("Einblenden") : t("Ausblenden")} onclick={() => setAll(u, "visible", allHidden)}><Icon name={allHidden ? "eyeOff" : "eye"} size={14} /></button>
          <button class="btn icon" class:active={allLocked} title={allLocked ? t("Entsperren") : t("Sperren")} onclick={() => setAll(u, "locked", !allLocked)}><Icon name={allLocked ? "lock" : "unlock"} size={14} /></button>
        </li>
      {:else}
        {@const o = row.o}
        <li
          class:sel={editor.selection.includes(o.id)}
          class:hidden={!o.visible}
          class:child={row.child}
          class:moving
          class:drop-before={dropBefore}
          class:drop-after={dropAfter}
          oncontextmenu={(e) => openMenu([o.id], e)}
        >
          {#if editing === o.id}
            <span class="main">
              {@render icon(o)}
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
              title={row.child ? t("Doppelklick oder F2: umbenennen · Rechtsklick: weitere Aktionen") : t("Doppelklick oder F2: umbenennen · Ziehen: Reihenfolge ändern · Rechtsklick: weitere Aktionen")}
              onpointerdown={(e) => !row.child && onDown(row.unit, e)}
              onclick={(e) => select(o.id, e)}
              ondblclick={() => startRename(o.id)}
            >
              {@render icon(o)}
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
      {/if}
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
  li.group.part {
    background: color-mix(in srgb, var(--accent) 6%, transparent);
  }
  li.group.sel {
    background: var(--accent-soft);
  }
  li.child {
    margin-left: 22px;
  }
  li.child::before {
    content: "";
    position: absolute;
    left: -10px;
    top: 0;
    bottom: 0;
    width: 1.5px;
    background: var(--border-strong);
  }
  .fold {
    width: 20px !important;
    margin-right: -6px;
  }
  .fold :global(svg) {
    transform: rotate(180deg);
    transition: transform 0.15s;
  }
  .fold.open :global(svg) {
    transform: rotate(-90deg);
  }
  .count {
    flex: none;
    margin-left: auto;
    padding: 0 6px;
    border-radius: 999px;
    background: var(--panel-2);
    color: var(--text-3);
    font-size: 10.5px;
  }
  .thumb {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    flex: none;
    border-radius: 4px;
    border: 1px solid var(--border);
    background: var(--panel-2);
    overflow: hidden;
  }
  .thumb svg {
    width: 18px;
    height: 18px;
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
