<script lang="ts">
  import { t } from "../i18n.svelte";
  let {
    label,
    value = $bindable(),
    min,
    max,
    step = 1,
    unit = "",
    disabled = false,
    display,
    hint,
    oninput,
  }: {
    label: string;
    value: number;
    min: number;
    max: number;
    step?: number;
    unit?: string;
    disabled?: boolean;
    display?: string;
    hint?: string;
    oninput?: (v: number) => void;
  } = $props();

  const pct = $derived(((value - min) / (max - min)) * 100);
</script>

<label class="slider" class:disabled title={hint ? t(hint) : undefined}>
  <span class="row">
    <span class="label">{t(label)}</span>
    <span class="value">{display ?? `${value}${unit}`}</span>
  </span>
  <input type="range" {min} {max} {step} {disabled} bind:value style:--pct="{pct}%" oninput={() => oninput?.(value)} />
</label>

<style>
  .slider {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .slider.disabled {
    opacity: 0.5;
  }
  .row {
    display: flex;
    justify-content: space-between;
    color: var(--text-2);
  }
  .value {
    color: var(--text);
    font-variant-numeric: tabular-nums;
  }
  input {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    height: 18px;
    margin: 0;
    background: transparent;
    cursor: pointer;
  }
  input::-webkit-slider-runnable-track {
    height: 4px;
    border-radius: 4px;
    background: linear-gradient(to right, var(--accent) var(--pct), var(--border) var(--pct));
  }
  input::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 14px;
    height: 14px;
    margin-top: -5px;
    border-radius: 50%;
    background: var(--panel);
    border: 2px solid var(--accent);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
  }
  input:disabled {
    cursor: default;
  }
</style>
