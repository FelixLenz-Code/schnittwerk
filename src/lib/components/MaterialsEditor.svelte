<script lang="ts">
  // Own materials in the settings: create from a preset, rename, change the
  // values, delete. Changes are stored right away.
  import { editor } from "../editor.svelte";
  import { uid } from "../geometry";
  import { t } from "../i18n.svelte";
  import { lastMaterialId, loadCustom, PRESETS, rememberMaterial, saveCustom, type Material } from "../materials";
  import Icon from "./Icon.svelte";
  import Slider from "./Slider.svelte";
  import Toggle from "./Toggle.svelte";

  let list = $state<Material[]>(loadCustom());
  let openId = $state<string | null>(null);
  let confirmDelete = $state<string | null>(null);
  let base = $state(PRESETS[0].id);

  function store() {
    saveCustom($state.snapshot(list) as Material[]);
  }

  function add() {
    const p = PRESETS.find((x) => x.id === base) ?? PRESETS[0];
    const entry: Material = { ...structuredClone(p), id: "c-" + uid(), name: `${t(p.name)} (${t("eigen")})`, custom: true };
    list = [...list, entry];
    store();
    openId = entry.id;
  }

  function remove(id: string) {
    if (confirmDelete !== id) {
      confirmDelete = id;
      return;
    }
    list = list.filter((m) => m.id !== id);
    store();
    confirmDelete = null;
    if (openId === id) openId = null;
    // Nothing may point at a deleted material.
    if (editor.material === id) editor.material = null;
    if (lastMaterialId() === id) rememberMaterial(PRESETS[0].id);
  }

  const summary = (m: Material) =>
    m.tool === "pen"
      ? `${t("Stift")} · ${t("Druck")} ${m.pressure} · ${t("Tempo")} ${m.speed}`
      : `${t("Tiefe")} ${m.depth} · ${t("Druck")} ${m.pressure} · ${t("Tempo")} ${m.speed}${m.passes > 1 ? ` · ${m.passes}×` : ""}`;
</script>

{#if !list.length}
  <p class="muted">{t("Noch keine eigenen Materialien. Lege eins auf Basis einer Vorlage an – oder speichere die Werte nach einem Testschnitt.")}</p>
{/if}

<ul>
  {#each list as m (m.id)}
    <li class:open={openId === m.id}>
      <div class="head">
        <button class="title" onclick={() => (openId = openId === m.id ? null : m.id)} aria-expanded={openId === m.id}>
          <span class="chev"><Icon name="chevronLeft" size={11} /></span>
          <span class="txt"><b>{m.name}</b><small>{summary(m)}</small></span>
        </button>
        <button class="btn icon del" class:confirm={confirmDelete === m.id} title={confirmDelete === m.id ? t("Noch einmal klicken zum Löschen") : t("Material löschen")} aria-label={t("Material löschen")} onclick={() => remove(m.id)} onblur={() => confirmDelete === m.id && (confirmDelete = null)}>
          <Icon name="trash" size={14} />
        </button>
      </div>
      {#if openId === m.id}
        <div class="edit">
          <label class="name-row">
            <span>{t("Name")}</span>
            <input class="field" bind:value={m.name} onchange={store} />
          </label>
          <label class="name-row">
            <span>{t("Werkzeug")}</span>
            <select class="field" bind:value={m.tool} onchange={store}>
              <option value="blade">{t("AutoBlade (Messer)")}</option>
              <option value="pen">{t("Stift")}</option>
            </select>
          </label>
          {#if m.tool === "blade"}
            <Slider label="Messertiefe" bind:value={m.depth} min={1} max={10} oninput={store} />
          {/if}
          <Slider label="Druck" bind:value={m.pressure} min={1} max={33} oninput={store} />
          <Slider label="Geschwindigkeit" bind:value={m.speed} min={1} max={30} oninput={store} />
          <Slider label="Durchgänge" bind:value={m.passes} min={1} max={5} unit="×" oninput={store} />
          {#if m.tool === "blade"}
            <Slider label="Überschnitt" bind:value={m.overcut} min={0} max={2} step={0.1} unit=" mm" oninput={store} />
          {/if}
          <Toggle label="Spiegeln (Flexfolie)" bind:checked={m.mirror} onchange={store} />
        </div>
      {/if}
    </li>
  {/each}
</ul>

<div class="add">
  <select class="field" bind:value={base} title={t("Werte dieser Vorlage übernehmen")}>
    {#each PRESETS as p}<option value={p.id}>{t(p.name)}</option>{/each}
  </select>
  <button class="btn outline" onclick={add}><Icon name="plus" /> {t("Neues Material")}</button>
</div>

<style>
  .muted {
    margin: 0;
    color: var(--text-3);
    font-size: 12.5px;
    line-height: 1.45;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  li {
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--panel);
  }
  li.open {
    border-color: var(--border-strong);
  }
  .head {
    display: flex;
    align-items: center;
    padding-right: 4px;
  }
  .title {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border: 0;
    background: none;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .chev {
    display: inline-flex;
    color: var(--text-3);
    transform: rotate(180deg);
    transition: transform 0.15s;
  }
  li.open .chev {
    transform: rotate(-90deg);
  }
  .txt {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }
  .txt b {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }
  .txt small {
    color: var(--text-3);
    font-size: 11.5px;
    font-variant-numeric: tabular-nums;
  }
  .del {
    color: var(--text-3);
  }
  .del:hover,
  .del.confirm {
    color: var(--danger);
  }
  .del.confirm {
    background: color-mix(in srgb, var(--danger) 12%, transparent);
  }
  .edit {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 4px 12px 12px;
    border-top: 1px solid var(--border);
    padding-top: 10px;
  }
  .name-row {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--text-2);
  }
  .name-row span {
    width: 80px;
    flex: none;
  }
  .name-row .field {
    flex: 1;
    min-width: 0;
  }
  .add {
    display: flex;
    gap: 6px;
  }
  .add select {
    flex: 1;
    min-width: 0;
  }
</style>
