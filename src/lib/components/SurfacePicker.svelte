<script lang="ts">
  // "What am I working on?" – mat, paper, print & cut page, roll or a free
  // size, in one place: a chip with a popover at the top left of the canvas.
  import { customMat, editor, matKindOf, MATS, PAGES, rollMat, ROLL_MAX, type PageName } from "../editor.svelte";
  import { fmt } from "../geometry";
  import { t } from "../i18n.svelte";
  import Icon from "./Icon.svelte";

  let open = $state(false);
  let root = $state<HTMLElement>();

  const kind = $derived(matKindOf(editor.mat));
  const pc = $derived(editor.printCut.enabled);

  /** Current choice as a key: mat/page name, "pc:A4", "roll" or "custom". */
  const current = $derived(pc ? `pc:${editor.printCut.page}` : kind === "roll" ? "roll" : kind === "custom" ? "custom" : editor.mat.name);

  const label = $derived.by(() => {
    if (pc) return `${editor.printCut.page} · Print & Cut`;
    if (kind === "roll") return `${t("Rolle ohne Matte")} · ${fmt(editor.mat.h, 0)} mm`;
    if (kind === "custom") return `${t("Eigene Größe")} · ${fmt(editor.mat.w, 0)} × ${fmt(editor.mat.h, 0)} mm`;
    return t(editor.mat.name);
  });
  const icon = $derived(pc ? "sticker" : kind === "roll" ? "scroll" : kind === "page" ? "card" : "grid");

  const groups = [
    { title: "Schneidematte", items: MATS.filter((m) => m.kind === "mat").map((m) => ({ key: m.name, label: m.name })) },
    { title: "Papier ohne Matte", items: MATS.filter((m) => m.kind === "page").map((m) => ({ key: m.name, label: m.name })) },
    {
      title: "Print & Cut (mit Passermarken)",
      items: (Object.keys(PAGES) as PageName[]).map((p) => ({ key: `pc:${p}`, label: p })),
    },
    {
      title: "Ohne Matte",
      items: [
        { key: "roll", label: "Rolle ohne Matte" },
        { key: "custom", label: "Eigene Größe (Kacheln)" },
      ],
    },
  ];

  function pick(key: string) {
    if (key === current) return;
    if (key.startsWith("pc:")) {
      editor.setPrintCut(true, key.slice(3) as PageName);
    } else {
      if (pc) editor.setPrintCut(false);
      if (key === "roll") editor.mat = rollMat(kind === "roll" ? editor.mat.h : 600);
      else if (key === "custom") editor.mat = customMat(Math.max(editor.mat.w, 300), Math.max(editor.mat.h, 400));
      else editor.mat = MATS.find((m) => m.name === key) ?? MATS[0];
      editor.fitMat();
    }
    editor.dirty = true;
    // Sizes are typed in the open popover; anything else closes it.
    if (key !== "roll" && key !== "custom") open = false;
  }

  function setSize(axis: "w" | "h", raw: string) {
    const v = parseFloat(raw.replace(",", "."));
    if (!Number.isFinite(v)) return;
    editor.mat =
      kind === "roll"
        ? rollMat(v)
        : customMat(axis === "w" ? v : editor.mat.w, axis === "h" ? v : editor.mat.h, editor.mat.roll, editor.mat.cut);
    editor.dirty = true;
    editor.fitMat();
  }
</script>

<svelte:window
  onpointerdown={(e) => open && root && !root.contains(e.target as Node) && (open = false)}
  onkeydown={(e) => open && e.key === "Escape" && (open = false)}
/>

{#snippet list()}
  {#each groups as g}
    <div class="group-title">{t(g.title)}</div>
    {#each g.items as it}
      <button class="item" class:on={current === it.key} role="menuitemradio" aria-checked={current === it.key} onclick={() => pick(it.key)}>
        <span class="radio"></span>
        <span class="lbl">{t(it.label)}</span>
      </button>
      {#if current === it.key && it.key === "roll"}
        <label class="size">
          {t("Länge")}
          <input class="field" type="number" min="50" max={ROLL_MAX} step="10" value={Math.round(editor.mat.h)} onchange={(e) => setSize("h", e.currentTarget.value)} />
          mm
        </label>
      {:else if current === it.key && it.key === "custom"}
        <label class="size">
          <input class="field" type="number" min="20" step="10" value={Math.round(editor.mat.w)} onchange={(e) => setSize("w", e.currentTarget.value)} aria-label={t("Breite")} />
          ×
          <input class="field" type="number" min="20" step="10" value={Math.round(editor.mat.h)} onchange={(e) => setSize("h", e.currentTarget.value)} aria-label={t("Höhe")} />
          mm
        </label>
      {/if}
    {/each}
  {/each}
{/snippet}

<div class="chip-wrap" bind:this={root} data-tour="area">
    <button class="chip" class:open onclick={() => (open = !open)} title={t("Worauf arbeitest du? Matte, Papier, Print & Cut oder Rolle")} aria-haspopup="menu" aria-expanded={open}>
      <Icon name={icon} size={14} />
      <span>{label}</span>
      <span class="chev"><Icon name="chevronLeft" size={11} /></span>
    </button>
    {#if open}
      <div class="pop" role="menu">{@render list()}</div>
    {/if}
  </div>

<style>
  .chip-wrap {
    position: relative;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 30px;
    padding: 0 10px 0 11px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--panel);
    box-shadow: var(--shadow);
    color: var(--text);
    font-size: 12.5px;
    font-weight: 500;
    cursor: pointer;
    white-space: nowrap;
  }
  .chip:hover,
  .chip.open {
    border-color: var(--border-strong);
  }
  .chip :global(svg) {
    color: var(--text-2);
  }
  .chev {
    display: inline-flex;
    transform: rotate(-90deg);
    transition: transform 0.15s;
  }
  .chip.open .chev {
    transform: rotate(90deg);
  }
  .pop {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    z-index: 30;
    width: 260px;
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
  .pop .group-title:first-child {
    padding-top: 2px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100%;
    height: 30px;
    padding: 0 8px;
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
  .lbl {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .size {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px 6px 29px;
    color: var(--text-2);
    font-size: 12px;
  }
  .size input {
    width: 72px;
    text-align: right;
  }
</style>
