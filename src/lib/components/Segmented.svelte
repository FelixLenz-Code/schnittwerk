<script lang="ts" generics="T extends string">
  import { t } from "../i18n.svelte";
  let {
    options,
    value,
    onchange,
    cols,
  }: { options: { value: T; label: string; hint?: string }[]; value: T; onchange: (v: T) => void; cols?: number } = $props();
</script>

<div class="seg" class:wrap={!!cols} style:--cols={cols} role="radiogroup">
  {#each options as o}
    <button
      type="button"
      role="radio"
      aria-checked={o.value === value}
      class:on={o.value === value}
      title={o.hint ? t(o.hint) : undefined}
      onclick={() => onchange(o.value)}>{t(o.label)}</button
    >
  {/each}
</div>

<style>
  .seg {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: 1fr;
    gap: 2px;
    padding: 3px;
    border-radius: var(--radius-sm);
    background: var(--panel-2);
    border: 1px solid var(--border);
  }
  .seg.wrap {
    grid-auto-flow: row;
    grid-template-columns: repeat(var(--cols), 1fr);
  }
  button {
    height: 26px;
    padding: 0 6px;
    border: 0;
    border-radius: 5px;
    background: transparent;
    color: var(--text-2);
    cursor: pointer;
    font-size: 12px;
    white-space: nowrap;
  }
  button:hover {
    color: var(--text);
  }
  button.on {
    background: var(--panel);
    color: var(--text);
    font-weight: 600;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.12);
  }
</style>
