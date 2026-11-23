<script lang="ts">
  let {
    label,
    checked = $bindable(),
    hint,
    disabled = false,
    onchange,
  }: { label: string; checked: boolean; hint?: string; disabled?: boolean; onchange?: (v: boolean) => void } = $props();
</script>

<label class="toggle" class:disabled title={hint}>
  <span>{label}</span>
  <input type="checkbox" bind:checked {disabled} onchange={() => onchange?.(checked)} />
  <span class="track"><span class="thumb"></span></span>
</label>

<style>
  .toggle {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    color: var(--text-2);
    cursor: pointer;
  }
  .toggle.disabled {
    opacity: 0.5;
    cursor: default;
  }
  input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
  .track {
    position: relative;
    width: 30px;
    height: 18px;
    flex: none;
    border-radius: 999px;
    background: var(--border-strong);
    transition: background 0.15s;
  }
  .thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.25);
    transition: transform 0.15s;
  }
  input:checked + .track {
    background: var(--accent);
  }
  input:checked + .track .thumb {
    transform: translateX(12px);
  }
  input:focus-visible + .track {
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
</style>
