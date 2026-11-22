<script lang="ts" generics="T extends string">
  let {
    options,
    value,
    onchange,
  }: { options: { value: T; label: string; hint?: string }[]; value: T; onchange: (v: T) => void } = $props();
</script>

<div class="seg" role="radiogroup">
  {#each options as o}
    <button
      type="button"
      role="radio"
      aria-checked={o.value === value}
      class:on={o.value === value}
      title={o.hint}
      onclick={() => onchange(o.value)}>{o.label}</button
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
