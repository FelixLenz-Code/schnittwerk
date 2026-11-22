<script lang="ts">
  import { editor } from "../editor.svelte";
  import { fonts, regenerate } from "../text.svelte";
  import type { PathObj, TextSpec } from "../types";
  import Segmented from "./Segmented.svelte";
  import Slider from "./Slider.svelte";
  import Toggle from "./Toggle.svelte";

  let { obj }: { obj: PathObj & { text: TextSpec } } = $props();

  fonts.load();
  let filter = $state("");
  const shown = $derived(
    filter ? fonts.families.filter((f) => f.toLowerCase().includes(filter.toLowerCase())).slice(0, 200) : fonts.families,
  );

  let touched = false;
  /** One undo step per editing burst. */
  function change<K extends keyof TextSpec>(key: K, value: TextSpec[K]) {
    if (!touched) {
      editor.checkpoint();
      touched = true;
      setTimeout(() => (touched = false), 800);
    }
    obj.text[key] = value;
    regenerate(obj.id);
  }
</script>

<section>
  <h3 class="section-title">Text</h3>
  <div class="stack">
    <textarea class="field area" rows="3" value={obj.text.content} oninput={(e) => change("content", e.currentTarget.value)}></textarea>
    <div class="fontpick">
      <input class="field" placeholder="Schrift suchen …" bind:value={filter} />
      <select class="field" size="6" value={obj.text.family} onchange={(e) => change("family", e.currentTarget.value)}>
        {#if !shown.includes(obj.text.family)}<option value={obj.text.family}>{obj.text.family}</option>{/if}
        {#each shown as f}<option value={f} style:font-family={f}>{f}</option>{/each}
      </select>
    </div>
    <label class="num">
      <span>Größe</span>
      <input class="field" type="number" min="1" step="1" value={obj.text.size} onchange={(e) => change("size", Math.max(1, +e.currentTarget.value || 20))} />
      <em>mm</em>
    </label>
    <div class="row">
      <button class="btn outline style" class:active={obj.text.bold} onclick={() => change("bold", !obj.text.bold)}><b>F</b></button>
      <button class="btn outline style" class:active={obj.text.italic} onclick={() => change("italic", !obj.text.italic)}><i>K</i></button>
      <div class="grow">
        <Segmented
          value={obj.text.align}
          onchange={(v) => change("align", v)}
          options={[
            { value: "start", label: "Links" },
            { value: "middle", label: "Mitte" },
            { value: "end", label: "Rechts" },
          ]}
        />
      </div>
    </div>
    <Slider label="Zeichenabstand" value={obj.text.spacing} min={-2} max={10} step={0.1} unit=" mm" oninput={(v) => change("spacing", v)} />
    {#if obj.text.content.includes("\n")}
      <Slider label="Zeilenabstand" value={obj.text.lineHeight} min={0.7} max={2.5} step={0.05} unit="×" oninput={(v) => change("lineHeight", v)} />
    {/if}
    <Toggle label="Verschweißen" checked={obj.text.weld} onchange={(v) => change("weld", v)} hint="Überlappende Buchstaben zu einer Kontur verbinden – wichtig für Schreibschrift" />
  </div>
</section>

<style>
  section {
    padding: 14px 16px;
    border-bottom: 1px solid var(--border);
  }
  .stack {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .area {
    height: auto;
    padding: 6px 8px;
    resize: vertical;
    font: inherit;
    user-select: text;
  }
  .fontpick {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .fontpick select {
    height: auto;
    padding: 4px;
  }
  .num {
    position: relative;
    display: flex;
    align-items: center;
  }
  .num span {
    position: absolute;
    left: 8px;
    color: var(--text-3);
    font-size: 11px;
    font-weight: 600;
  }
  .num em {
    position: absolute;
    right: 8px;
    color: var(--text-3);
    font-style: normal;
    font-size: 11px;
  }
  .num .field {
    padding-left: 48px;
    padding-right: 30px;
  }
  .row {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .style {
    width: 32px;
    padding: 0;
    justify-content: center;
  }
  .grow {
    flex: 1;
  }
</style>
