<script lang="ts">
  import { editor } from "../editor.svelte";
  import { fonts, regenerate } from "../text.svelte";
  import type { PathObj, TextSpec } from "../types";
  import Segmented from "./Segmented.svelte";
  import Slider from "./Slider.svelte";
  import Toggle from "./Toggle.svelte";
  import { t } from "../i18n.svelte";
  import { parsePath, reversePath, serializePath } from "../pathdata";

  let { obj }: { obj: PathObj & { text: TextSpec } } = $props();

  fonts.load();
  let filter = $state("");
  const shown = $derived(
    filter ? fonts.families.filter((f) => f.toLowerCase().includes(filter.toLowerCase())).slice(0, 200) : fonts.families,
  );

  const curveKind = $derived(obj.text.curve?.kind ?? "none");

  /** Straight / arc: the box keeps its centre. */
  function setCurve(v: "none" | "arcTop" | "arcBottom" | "custom") {
    if (v === curveKind || v === "custom") return;
    editor.checkpoint();
    const radius = obj.text.curve && obj.text.curve.kind !== "custom" ? obj.text.curve.radius : Math.max(15, obj.text.size * 2.5);
    obj.text.curve = v === "none" ? null : { kind: v, radius };
    if (v === "none") obj.text.path = null;
    obj.text.pathOffset = 0;
    regenerate(obj.id, "center");
  }

  function flipSide() {
    if (!obj.text.path) return;
    editor.checkpoint();
    obj.text.path = serializePath(reversePath(parsePath(obj.text.path)));
    regenerate(obj.id, "exact");
  }

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
  <h3 class="section-title">{t("Text")}</h3>
  <div class="stack">
    <textarea class="field area" rows="3" value={obj.text.content} oninput={(e) => change("content", e.currentTarget.value)}></textarea>
    <div class="fontpick">
      <input class="field" placeholder={t("Schrift suchen …")} bind:value={filter} />
      <select class="field" size="6" value={obj.text.family} onchange={(e) => change("family", e.currentTarget.value)}>
        {#if !shown.includes(obj.text.family)}<option value={obj.text.family}>{obj.text.family}</option>{/if}
        {#each shown as f}<option value={f} style:font-family={f}>{f}</option>{/each}
      </select>
    </div>
    <label class="num">
      <span>{t("Größe")}</span>
      <input class="field" type="number" min="1" step="1" value={obj.text.size} onchange={(e) => change("size", Math.max(1, +e.currentTarget.value || 20))} />
      <em>mm</em>
    </label>
    <div class="row">
      <button class="btn outline style" class:active={obj.text.bold} onclick={() => change("bold", !obj.text.bold)}><b>{t("F")}</b></button>
      <button class="btn outline style" class:active={obj.text.italic} onclick={() => change("italic", !obj.text.italic)}><i>{t("K")}</i></button>
      {#if curveKind === "none" || curveKind === "custom"}
        <div class="grow">
          <Segmented
            value={obj.text.align}
            onchange={(v) => change("align", v)}
            options={curveKind === "custom"
              ? [
                  { value: "start", label: "Anfang" },
                  { value: "middle", label: "Mitte" },
                  { value: "end", label: "Ende" },
                ]
              : [
                  { value: "start", label: "Links" },
                  { value: "middle", label: "Mitte" },
                  { value: "end", label: "Rechts" },
                ]}
          />
        </div>
      {/if}
    </div>
    <div class="curve">
      <Segmented
        value={curveKind}
        onchange={setCurve}
        options={[
          { value: "none", label: "Gerade" },
          { value: "arcTop", label: "Bogen ⌒", hint: "Text oben auf einem Kreis" },
          { value: "arcBottom", label: "Bogen ⌣", hint: "Text unten an einem Kreis" },
          ...(curveKind === "custom" ? [{ value: "custom" as const, label: "Pfad", hint: "Text folgt einem Pfad der Arbeitsfläche" }] : []),
        ]}
      />
    </div>
    {#if obj.text.curve && obj.text.curve.kind !== "custom"}
      {@const c = obj.text.curve}
      <Slider label="Radius" value={c.radius} min={5} max={300} step={1} unit=" mm" oninput={(v) => change("curve", { kind: c.kind, radius: v })} />
    {/if}
    {#if obj.text.curve}
      <Slider label="Verschieben" value={obj.text.pathOffset ?? 0} min={-50} max={50} step={0.5} unit=" %" oninput={(v) => change("pathOffset", v)} hint="Text entlang des Pfads verschieben" />
    {/if}
    {#if curveKind === "custom"}
      <button class="btn outline" onclick={flipSide} title={t("Laufrichtung umkehren – der Text wechselt auf die andere Seite der Linie")}>{t("Seite wechseln")}</button>
      <p class="muted">{t("Text folgt dem gewählten Pfad. Für einen anderen Pfad: Text und Pfad auswählen → „Text auf Pfad“.")}</p>
    {:else if curveKind === "none"}
      <p class="muted">{t("Auf eine Form setzen: Text und Pfad zusammen auswählen → „Text auf Pfad“.")}</p>
    {/if}
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
  .muted {
    margin: 0;
    color: var(--text-3);
    font-size: 12px;
  }
</style>
