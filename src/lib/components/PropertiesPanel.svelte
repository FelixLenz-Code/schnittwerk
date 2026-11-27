<script lang="ts">
  import { api } from "../api";
  import { editor } from "../editor.svelte";
  import { aabb, fmt, uid } from "../geometry";
  import { pathMatrix, plotter } from "../plotter.svelte";
  import type { ImageObj, Obj, PathObj } from "../types";
  import Icon from "./Icon.svelte";
  import Toggle from "./Toggle.svelte";
  import TextEditor from "./TextEditor.svelte";
  import { nodes } from "../nodes.svelte";
  import { refreshShape, rememberSpec, SHAPE_NAMES } from "../shapes";
  import Slider from "./Slider.svelte";
  import { t } from "../i18n.svelte";
  import { addWeeding, BOOL_OPS, combine, CUT_TYPES, flip, frameSelection, nestSelection, putTextOnPath, setCutColor, textOnPathPair } from "../actions";
  import { perspective } from "../perspective.svelte";
  import { openSection, ui } from "../ui.svelte";
  import PixScanSection from "./PixScanSection.svelte";
  import Section from "./Section.svelte";

  const single = $derived(editor.selected.length === 1 ? editor.selected[0] : null);
  const box = $derived(aabb(editor.selected));

  /** Applies a numeric field edit as one undo step. */
  function setField(o: Obj, key: "x" | "y" | "w" | "h" | "rot", raw: string) {
    const v = parseFloat(raw.replace(",", "."));
    if (!Number.isFinite(v)) return;
    if (key === "w" || key === "h") return editor.setSize(o, key, v);
    editor.checkpoint("Position ändern");
    if (key === "rot") {
      o.rot = ((v % 360) + 360) % 360;
    } else if (key === "x" || key === "y") {
      o[key] = v;
    }
  }

  function setColor(o: Obj, color: string) {
    if (o.type !== "path") return;
    editor.checkpoint("Farbe ändern");
    o.color = color;
  }

  // ---------- offset ----------
  let offsetMm = $state(2);
  let offsetBusy = $state(false);
  const selPaths = $derived(editor.selected.filter((o): o is PathObj => o.type === "path"));

  async function offsetSelected() {
    if (!selPaths.length) return;
    offsetBusy = true;
    try {
      const inputs = selPaths.map((o) => ({ d: o.d, matrix: pathMatrix(o, null) }));
      const r = await api.offsetPaths(inputs, offsetMm, 50);
      if (!r.contours) throw new Error("Versatz ergibt keine Kontur (zu stark nach innen?).");
      const [x, y, w, h] = r.bbox;
      const obj: PathObj = {
        id: uid(), type: "path", name: t("Versatz {d} mm", { d: fmt(offsetMm) }), d: r.d, bbox: r.bbox, color: "#3e63dd", fill: false,
        x, y, w, h, rot: 0, flipX: false, flipY: false, visible: true, locked: false,
      };
      editor.add([obj]);
    } catch (e) {
      editor.error(e);
    } finally {
      offsetBusy = false;
    }
  }

  let boolBusy = $state(false);
  async function runCombine(op: (typeof BOOL_OPS)[number]["op"]) {
    boolBusy = true;
    await combine(op);
    boolBusy = false;
  }
  const textOnPath = $derived(textOnPathPair());

  // ---------- sheet tools ----------
  let weedMargin = $state(3);
  let weedSpacing = $state(20);
  let weedDir = $state<"h" | "v" | "hv" | "none">("h");
  let gap = $state(3);
  let nestShapes = $state(true);
  let nesting = $state(false);
  async function nest(fill: boolean) {
    nesting = true;
    await nestSelection(fill, gap, nestShapes);
    nesting = false;
  }

  // ---------- pictures ----------
  const images = $derived(editor.selected.filter((o): o is ImageObj => o.type === "image"));
  const onlyImages = $derived(images.length > 0 && images.length === editor.selected.length);
  let showPix = $state(false);
  // A PixScan photo keeps its section open; another picture starts closed.
  $effect(() => {
    showPix = !!(single?.type === "image" && single.pixscan);
  });
  // Selecting something else ends the PixScan corner tool.
  $effect(() => {
    if (perspective.objId && single?.id !== perspective.objId) perspective.cancel();
  });

  // ---------- next steps (after tracing / stickers) ----------
  const next = $derived(ui.nextSteps && ui.nextSteps.sel === editor.selection.join(",") ? ui.nextSteps : null);

  // ---------- shape parameters ----------
  let shapeTouched = false;
  function setShape(o: PathObj, spec: NonNullable<PathObj["shape"]>) {
    if (!shapeTouched) {
      editor.checkpoint("Form ändern");
      shapeTouched = true;
      setTimeout(() => (shapeTouched = false), 800);
    }
    o.shape = spec;
    refreshShape(o);
    rememberSpec(spec);
  }

  // ---------- simplify ----------
  let simplifyTol = $state(0.1);
  let simplifyBusy = $state(false);

  async function simplify(o: PathObj) {
    simplifyBusy = true;
    try {
      // Tolerance in mm → path units.
      const scale = (o.w / (o.bbox[2] || 1) + o.h / (o.bbox[3] || 1)) / 2;
      const r = await api.simplifyPath(o.d, simplifyTol / (scale || 1), 35);
      if (r.nodesAfter >= r.nodesBefore) {
        editor.toast("Weniger Knoten geht bei dieser Abweichung nicht.");
        return;
      }
      editor.checkpoint("Knoten reduzieren");
      o.d = r.d;
      editor.toast(t("{a} → {b} Knoten", { a: r.nodesBefore, b: r.nodesAfter }), "ok");
    } catch (e) {
      editor.error(e);
    } finally {
      simplifyBusy = false;
    }
  }

  // ---------- several objects ----------
  /** Moves the whole selection so its box starts at the value. */
  function moveSelection(axis: "x" | "y", raw: string) {
    const v = parseFloat(raw.replace(",", "."));
    const b = aabb(editor.selected);
    if (!Number.isFinite(v) || !b) return;
    const d = v - (axis === "x" ? b.x0 : b.y0);
    editor.checkpoint("Verschieben");
    for (const o of editor.selected) if (!o.locked) o[axis] += d;
  }

  /** Common value of all selected objects (shown as placeholder), else "–". */
  function sameValue(key: "w" | "h" | "rot"): string {
    const vals = editor.selected.map((o) => Math.round(o[key] * 10) / 10);
    return vals.every((v) => v === vals[0]) ? fmt(vals[0], 1) : "–";
  }

  /** Sets width, height or rotation of every selected object. */
  function setEach(key: "w" | "h" | "rot", raw: string, input: HTMLInputElement) {
    const v = parseFloat(raw.replace(",", "."));
    input.value = "";
    if (!Number.isFinite(v) || (key !== "rot" && v <= 0)) return;
    editor.checkpoint(key === "rot" ? "Drehen" : "Größe ändern");
    for (const o of editor.selected) {
      if (o.locked) continue;
      if (key === "rot") {
        o.rot = ((v % 360) + 360) % 360;
        continue;
      }
      const ratio = o.h / o.w;
      const cx = o.x + o.w / 2, cy = o.y + o.h / 2;
      if (key === "w") {
        o.w = v;
        if (editor.keepRatio) o.h = v * ratio;
      } else {
        o.h = v;
        if (editor.keepRatio) o.w = v / ratio;
      }
      o.x = cx - o.w / 2;
      o.y = cy - o.h / 2;
      if (o.type === "path") refreshShape(o);
    }
  }

  const CUT_COLORS = ["#e5484d", "#3e63dd", "#30a46c", "#f5a524", "#8e4ec6", "#1b1d22"];
</script>

{#if next}
  <div class="next">
    <div class="next-head">
      <span class="ok"><Icon name="check" size={13} stroke={2.4} /></span>
      <strong>{next.kind === "sticker" ? t("Sticker angelegt") : t("Schnittlinien erstellt")}</strong>
      <button class="btn icon close" title={t("Ausblenden")} aria-label={t("Ausblenden")} onclick={() => (ui.nextSteps = null)}><Icon name="x" size={13} /></button>
    </div>
    <p>{t("Wie geht es weiter?")}</p>
    <div class="next-actions">
      <button class="btn outline" onclick={() => openSection("fill")}><Icon name="copy" /> {t("Bogen füllen")}</button>
      {#if editor.printCut.enabled}
        <button class="btn outline" onclick={() => (plotter.printOpen = true)}><Icon name="printer" /> {t("Drucken")}</button>
      {/if}
      <button class="btn primary" onclick={() => (plotter.dialogOpen = true)}><Icon name="scissors" /> {t("Schneiden")}</button>
    </div>
  </div>
{/if}

{#if single}
  <section class="title">
    <div class="title-row">
      <Icon name={single.type === "image" ? "imagePlus" : single.type === "path" && single.text ? "type" : "scissors"} />
      <input
        class="name"
        value={single.name}
        onchange={(e) => {
          editor.checkpoint("Umbenennen");
          single.name = e.currentTarget.value;
        }}
      />
    </div>
  </section>
{:else if box}
  <section class="title">
    <h3 class="section-title">{t("{n} Objekte", { n: editor.selected.length })}</h3>
    <p class="muted no-gap">{fmt(box.x1 - box.x0)} × {fmt(box.y1 - box.y0)} mm</p>
  </section>
{/if}

{#if onlyImages}
  <!-- Pictures: the two ways forward come first. -->
  <section class="paths">
    {#if images.length === 1}
      <button class="way" onclick={() => (editor.rightTab = "trace")}>
        <span class="way-icon"><Icon name="wand" size={18} /></span>
        <span><b>{t("In Schnittlinien umwandeln")}</b><small>{t("Zeichnung, Logo oder Schrift tracen – für Folie, Papier und Stift")}</small></span>
        <span class="arrow"><Icon name="arrowRight" size={14} /></span>
      </button>
    {/if}
    <button class="way" onclick={() => (editor.rightTab = "sticker")}>
      <span class="way-icon"><Icon name="sparkle" size={18} /></span>
      <span>
        <b>{images.length > 1 ? t("Sticker-Bogen erstellen") : t("Als Sticker")}</b>
        <small>{images.length > 1 ? t("Alle ausgewählten Bilder bekommen eine Kontur mit Rand") : t("Kontur mit Rand zum Drucken und Ausschneiden (Print & Cut)")}</small>
      </span>
      <span class="arrow"><Icon name="arrowRight" size={14} /></span>
    </button>
    {#if single?.type === "image"}
      <label class="range-row">
        <span>{t("Deckkraft")}</span>
        <input type="range" min="0.1" max="1" step="0.05" bind:value={single.opacity} />
      </label>
      {#if showPix}
        <div class="pix"><PixScanSection img={single} /></div>
      {:else}
        <button class="link" onclick={() => (showPix = true)}>{t("Ist das ein Foto der PixScan-Matte?")}</button>
      {/if}
    {/if}
  </section>
{/if}

{#if single}
  <Section id="pos" title={t("Position & Größe")}>
    <div class="grid">
      {#each [["x", "X"], ["y", "Y"], ["w", "B"], ["h", "H"]] as [key, label]}
        <label class="num">
          <span>{t(label)}</span>
          <input class="field" value={fmt(single[key as "x"], 2)} disabled={single.locked} onchange={(e) => setField(single, key as "x", e.currentTarget.value)} />
          <em>mm</em>
        </label>
      {/each}
      <label class="num">
        <span>∠</span>
        <input class="field" value={fmt(single.rot, 1)} disabled={single.locked} onchange={(e) => setField(single, "rot", e.currentTarget.value)} />
        <em>°</em>
      </label>
      <button class="btn icon outline" class:active={editor.keepRatio} title={t("Seitenverhältnis beibehalten")} onclick={() => (editor.keepRatio = !editor.keepRatio)}>
        <Icon name={editor.keepRatio ? "lock" : "unlock"} />
      </button>
    </div>
  </Section>
{:else if box}
  <Section id="pos-multi" title={t("Position & Größe")}>
    <div class="grid multi">
      <label class="num" title={t("Verschiebt die ganze Auswahl")}>
        <span>X</span>
        <input class="field" value={fmt(box.x0, 2)} onchange={(e) => moveSelection("x", e.currentTarget.value)} />
        <em>mm</em>
      </label>
      <label class="num" title={t("Verschiebt die ganze Auswahl")}>
        <span>Y</span>
        <input class="field" value={fmt(box.y0, 2)} onchange={(e) => moveSelection("y", e.currentTarget.value)} />
        <em>mm</em>
      </label>
      <label class="num" title={t("Breite jedes Objekts")}>
        <span>{t("B")}</span>
        <input class="field" placeholder={sameValue("w")} onchange={(e) => setEach("w", e.currentTarget.value, e.currentTarget)} />
        <em>mm</em>
      </label>
      <label class="num" title={t("Höhe jedes Objekts")}>
        <span>{t("H")}</span>
        <input class="field" placeholder={sameValue("h")} onchange={(e) => setEach("h", e.currentTarget.value, e.currentTarget)} />
        <em>mm</em>
      </label>
      <label class="num" title={t("Drehung jedes Objekts")}>
        <span>∠</span>
        <input class="field" placeholder={sameValue("rot")} onchange={(e) => setEach("rot", e.currentTarget.value, e.currentTarget)} />
        <em>°</em>
      </label>
      <button class="btn icon outline" class:active={editor.keepRatio} title={t("Seitenverhältnis beibehalten")} onclick={() => (editor.keepRatio = !editor.keepRatio)}>
        <Icon name={editor.keepRatio ? "lock" : "unlock"} />
      </button>
    </div>
    {#if textOnPath}
      <button class="btn primary top-gap" onclick={putTextOnPath} title={t("Der Text folgt der Linie des anderen Pfads")}>{t("Text auf Pfad")}</button>
    {/if}
  </Section>
{/if}

{#if single?.type === "path" && single.shape && single.shape.kind !== "ellipse" && single.shape.kind !== "heart"}
  {@const sh = single.shape}
  <Section id="shape" title={t(SHAPE_NAMES[sh.kind])}>
    <div class="stack">
      {#if sh.kind === "rect"}
        <Slider label="Eckenradius" value={sh.radius} min={0} max={Math.round(Math.min(single.w, single.h) / 2)} step={0.5} unit=" mm" oninput={(v) => setShape(single as PathObj, { ...sh, radius: v })} />
      {:else if sh.kind === "polygon"}
        <Slider label="Ecken" value={sh.sides} min={3} max={12} oninput={(v) => setShape(single as PathObj, { ...sh, sides: v })} />
      {:else if sh.kind === "star"}
        <Slider label="Zacken" value={sh.points} min={3} max={20} oninput={(v) => setShape(single as PathObj, { ...sh, points: v })} />
        <Slider label="Innenradius" value={Math.round(sh.inner * 100)} min={10} max={90} unit=" %" oninput={(v) => setShape(single as PathObj, { ...sh, inner: v / 100 })} />
      {/if}
    </div>
  </Section>
{/if}

{#if single?.type === "path" && single.text}
  <TextEditor obj={single as PathObj & { text: NonNullable<PathObj["text"]> }} />
{/if}

{#if selPaths.length}
  <Section id="line" title={t("Schnittlinie")}>
    <div class="cut-types">
      {#each CUT_TYPES as ct}
        <button class="btn outline ctype" class:active={selPaths.every((o) => o.color.toLowerCase() === ct.color)} title={t(ct.hint)} onclick={() => setCutColor(ct.color)}>
          <span class="dot" style:background={ct.color}></span>{t(ct.label)}
        </button>
      {/each}
    </div>
    <p class="muted small">{t("Jede Farbe bekommt beim Schneiden eigene Einstellungen – angeschnitten wird zuerst.")}</p>
    {#if single?.type === "path"}
      <div class="colors">
        {#each CUT_COLORS as c}
          <button class="color" class:on={single.color === c} style:background={c} aria-label={t("Farbe {c}", { c })} onclick={() => setColor(single, c)}></button>
        {/each}
        <input type="color" value={single.color} onchange={(e) => setColor(single, e.currentTarget.value)} title={t("Eigene Farbe")} />
      </div>
      <div class="stack">
        <Toggle label="Füllung anzeigen" bind:checked={single.fill} hint="Nur Anzeige – geschnitten wird immer die Linie" />
        <label class="toggle-row">
          <span>{t("Schneiden")}</span>
          <input type="checkbox" checked={single.cut !== false} onchange={(e) => { editor.checkpoint("Schneiden an/aus"); single.cut = e.currentTarget.checked; }} />
        </label>
        <label class="toggle-row">
          <span>{t("Fläche drucken")}</span>
          <span class="row">
            {#if single.printFill}
              <input type="color" value={single.printFill} onchange={(e) => { editor.checkpoint("Druckfarbe"); single.printFill = e.currentTarget.value; }} />
            {/if}
            <input type="checkbox" checked={!!single.printFill} onchange={(e) => { editor.checkpoint("Druckfarbe"); single.printFill = e.currentTarget.checked ? "#ffffff" : null; }} />
          </span>
        </label>
      </div>
    {/if}
  </Section>
{/if}

{#if single?.type === "path"}
  <Section id="nodes" title={t("Knoten")}>
    {#if nodes.objId === single.id}
      <p class="muted small">{t("{n} Knoten · {s} ausgewählt", { n: nodes.count, s: nodes.sel.length })}</p>
      <div class="cut-types">
        <button class="btn outline" disabled={!nodes.sel.length} onclick={() => nodes.setSmooth(false)} title={t("Knick: Griffe unabhängig")}>{t("Ecke")}</button>
        <button class="btn outline" disabled={!nodes.sel.length} onclick={() => nodes.setSmooth(true)} title={t("Weicher Übergang: Griffe in einer Linie")}>{t("Glatt")}</button>
        <button class="btn outline" disabled={!nodes.sel.length} onclick={() => nodes.setStraight(true)} title={t("Abschnitt nach dem Knoten wird gerade")}>{t("Gerade")}</button>
        <button class="btn outline" disabled={!nodes.sel.length} onclick={() => nodes.setStraight(false)} title={t("Abschnitt nach dem Knoten wird zur Kurve")}>{t("Kurve")}</button>
        <button class="btn outline danger" disabled={!nodes.sel.length} onclick={() => nodes.deleteSelected()}>{t("Löschen")}</button>
        <button class="btn primary" onclick={() => nodes.stop()}><Icon name="check" /> {t("Fertig")}</button>
      </div>
      <p class="muted small">{t("Doppelklick auf die Linie fügt einen Knoten ein.")} <kbd>Alt</kbd> {t("beim Ziehen eines Griffs macht eine Ecke.")}</p>
    {:else}
      <button class="btn outline" disabled={single.locked} onclick={() => nodes.start(single as PathObj)} title={t("Auch per Doppelklick auf den Pfad")}>{t("Knoten bearbeiten")}</button>
      {#if !single.text && !single.shape}
        <div class="simplify">
          <Slider label="Vereinfachen" bind:value={simplifyTol} min={0.02} max={1} step={0.01} unit=" mm" hint="Erlaubte Abweichung von der Originalform" />
          <button class="btn outline" disabled={single.locked || simplifyBusy} onclick={() => simplify(single as PathObj)} title={t("Weniger Knoten, gleiche Form – schneidet ruhiger")}>
            {simplifyBusy ? t("Berechne …") : t("Knoten reduzieren")}
          </button>
        </div>
      {/if}
    {/if}
  </Section>
{/if}

<Section id="align" title={editor.selected.length === 1 && !editor.printCut.enabled ? t("Ausrichten auf Matte") : t("Ausrichten")}>
  <div class="row">
    <button class="btn icon" title={t("Links")} onclick={() => editor.align("left")}><Icon name="alignLeft" /></button>
    <button class="btn icon" title={t("Horizontal zentrieren")} onclick={() => editor.align("hcenter")}><Icon name="alignHCenter" /></button>
    <button class="btn icon" title={t("Rechts")} onclick={() => editor.align("right")}><Icon name="alignRight" /></button>
    <span class="sep"></span>
    <button class="btn icon" title={t("Oben")} onclick={() => editor.align("top")}><Icon name="alignTop" /></button>
    <button class="btn icon" title={t("Vertikal zentrieren")} onclick={() => editor.align("vcenter")}><Icon name="alignVCenter" /></button>
    <button class="btn icon" title={t("Unten")} onclick={() => editor.align("bottom")}><Icon name="alignBottom" /></button>
  </div>
  {#if editor.selectedUnits().length >= 3}
    <div class="row wrap top-gap">
      <button class="btn outline" title={t("Gleiche Abstände von links nach rechts")} onclick={() => editor.distribute("x")}><Icon name="distributeH" /> {t("Waagrecht verteilen")}</button>
      <button class="btn outline" title={t("Gleiche Abstände von oben nach unten")} onclick={() => editor.distribute("y")}><Icon name="distributeV" /> {t("Senkrecht verteilen")}</button>
    </div>
  {/if}
</Section>

<Section id="arrange" title={t("Anordnen")}>
  <div class="row wrap">
    <button class="btn outline" onclick={() => flip("x")}><Icon name="flipH" /> {t("Spiegeln")}</button>
    <button class="btn outline" onclick={() => flip("y")}><Icon name="flipV" /> {t("Kippen")}</button>
    <button class="btn icon outline" title={t("Nach vorne (Strg+↑)")} onclick={() => editor.restack(1)}><Icon name="arrowUp" /></button>
    <button class="btn icon outline" title={t("Nach hinten (Strg+↓)")} onclick={() => editor.restack(-1)}><Icon name="arrowDown" /></button>
  </div>
  <div class="row wrap" style="margin-top: 8px">
    {#if editor.selected.some((o) => o.group)}
      <button class="btn outline" title={t("Gruppierung aufheben (Strg+Shift+G)")} onclick={() => editor.ungroupSelected()}><Icon name="layers" /> {t("Lösen")}</button>
    {:else if editor.selected.length > 1}
      <button class="btn outline" title={t("Gruppieren (Strg+G)")} onclick={() => editor.groupSelected()}><Icon name="layers" /> {t("Gruppieren")}</button>
    {/if}
    <button class="btn outline" onclick={() => editor.duplicateSelected()}><Icon name="copy" /> {t("Duplizieren")}</button>
    <button class="btn outline danger" onclick={() => editor.removeSelected()}><Icon name="trash" /> {t("Löschen")}</button>
  </div>
</Section>

<!-- Less frequent tools, collapsed by default. -->
<div class="tools-head">{t("Werkzeuge")}</div>

<Section id="fill" title={t("Bogen füllen")} open={false}>
  <div class="row">
    <label class="num grow-field">
      <span>{t("Abstand")}</span>
      <input class="field pad-wide" type="number" min="0" step="0.5" bind:value={gap} />
      <em>mm</em>
    </label>
    <button class="btn outline" disabled={nesting} onclick={() => nest(true)} title={t("Bogen mit Kopien der Auswahl füllen")}>{t("Füllen")}</button>
    <button class="btn outline" disabled={nesting} onclick={() => nest(false)} title={t("Ausgewählte Teile platzsparend anordnen (ohne Kopien)")}>{t("Anordnen")}</button>
  </div>
  <div class="stack">
    <Toggle label="Formen verschachteln" bind:checked={nestShapes} hint="Echte Umrisse ineinander setzen und drehen statt eines starren Rasters" />
  </div>
  <p class="muted small">{editor.printCut.enabled ? t("Verteilt die Auswahl so oft wie möglich auf der Seite (ohne Markenzonen).") : t("Verteilt die Auswahl so oft wie möglich auf der Matte.")}</p>
</Section>

<Section id="frame" title={t("Rahmen")} open={false}>
  <button class="btn outline" onclick={frameSelection} title={t("Abgerundeter Rahmen mit 2 mm Abstand, wird durchgeschnitten")}>{t("Schnittrahmen um Auswahl")}</button>
</Section>

{#if selPaths.length >= 2}
  <Section id="bool" title={t("Pfade kombinieren")} open={false}>
    <div class="cut-types">
      {#each BOOL_OPS as b}
        <button class="btn outline" disabled={boolBusy} title={t(b.hint)} onclick={() => runCombine(b.op)}>{t(b.label)}</button>
      {/each}
    </div>
  </Section>
{/if}

{#if selPaths.length}
  <Section id="weed" title={t("Entgittern (Folie)")} open={false}>
    <div class="stack">
      <label class="toggle-row" title={t("Abstand des Rahmens zum Motiv")}>
        <span>{t("Rand")}</span>
        <span class="row unit-field"><input class="field" type="number" min="0.5" step="0.5" bind:value={weedMargin} /> mm</span>
      </label>
      <label class="toggle-row">
        <span>{t("Linien")}</span>
        <select class="field weed-select" bind:value={weedDir}>
          <option value="h">{t("Waagrecht")}</option>
          <option value="v">{t("Senkrecht")}</option>
          <option value="hv">{t("Beides")}</option>
          <option value="none">{t("Keine")}</option>
        </select>
      </label>
      {#if weedDir !== "none"}
        <label class="toggle-row" title={t("Abstand zwischen den Linien")}>
          <span>{t("Linienabstand")}</span>
          <span class="row unit-field"><input class="field" type="number" min="3" step="1" bind:value={weedSpacing} /> mm</span>
        </label>
      {/if}
      <button class="btn outline" onclick={() => addWeeding({ margin: weedMargin, spacing: weedSpacing, dir: weedDir })} title={t("Rahmen um das Motiv und Linien durch den Abfall – die überschüssige Folie lässt sich in Streifen abziehen")}>{t("Entgitterungshilfe anlegen")}</button>
    </div>
    <p class="muted small">{t("Linien halten Abstand zum Motiv und laufen auch durch Innenräume (z. B. im „O“).")}</p>
  </Section>

  <Section id="offset" title={t("Kontur versetzen")} open={false}>
    <div class="row">
      <label class="num grow-field">
        <span>±</span>
        <input class="field" type="number" step="0.5" bind:value={offsetMm} />
        <em>mm</em>
      </label>
      <button class="btn outline" disabled={offsetBusy} onclick={offsetSelected}>{offsetBusy ? t("Berechne …") : t("Neue Kontur")}</button>
    </div>
    <p class="muted small">{t("Positiv = nach außen, negativ = nach innen. Mehrere Pfade werden zu einer Kontur verbunden.")}</p>
  </Section>
{/if}

<style>
  section {
    padding: 14px 16px;
    border-bottom: 1px solid var(--border);
  }
  .title-row {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-2);
  }
  .name {
    flex: 1;
    min-width: 0;
    height: 28px;
    padding: 0 6px;
    border: 1px solid transparent;
    border-radius: 6px;
    background: transparent;
    font-weight: 600;
    outline: none;
  }
  .name:hover {
    border-color: var(--border);
  }
  .name:focus {
    border-color: var(--accent);
    background: var(--panel-2);
  }
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
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
    padding-left: 24px;
    padding-right: 28px;
    font-variant-numeric: tabular-nums;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .row.wrap {
    flex-wrap: wrap;
    gap: 6px;
  }
  .sep {
    width: 1px;
    height: 18px;
    background: var(--border);
    margin: 0 4px;
  }
  .stack {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin-top: 12px;
  }
  .stack:first-of-type {
    margin-top: 0;
  }
  .weed-select {
    width: 130px;
  }
  .unit-field {
    align-items: center;
    gap: 6px;
    color: var(--text-3);
  }
  .unit-field input {
    width: 96px;
    text-align: right;
  }
  .colors {
    display: flex;
    gap: 6px;
    align-items: center;
    margin: 12px 0 4px;
  }
  .color {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    border: 2px solid var(--panel);
    box-shadow: 0 0 0 1px var(--border-strong);
    cursor: pointer;
    padding: 0;
  }
  .color.on {
    box-shadow: 0 0 0 2px var(--accent);
  }
  input[type="color"] {
    width: 26px;
    height: 26px;
    padding: 0;
    border: 0;
    background: none;
    cursor: pointer;
  }
  .range-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    color: var(--text-2);
  }
  .range-row input {
    width: 150px;
    accent-color: var(--accent);
  }
  .muted {
    color: var(--text-3);
    margin: 0;
  }
  .muted.small {
    margin-top: 8px;
    font-size: 12px;
  }
  .grid.multi {
    margin-top: 10px;
  }
  .grow-field {
    flex: 1;
  }
  .simplify {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 12px;
  }
  .top-gap {
    margin-top: 10px;
  }
  .pad-wide {
    padding-left: 60px !important;
  }
  .cut-types {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .ctype {
    padding: 0 8px;
  }
  .ctype .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
  }
  .toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    color: var(--text-2);
  }
  .toggle-row input[type="checkbox"] {
    width: 16px;
    height: 16px;
    accent-color: var(--accent);
  }
  .title {
    padding-bottom: 12px;
  }
  .no-gap {
    margin: 2px 0 0;
  }
  /* "What next?" after tracing / stickers */
  .next {
    margin: 12px 12px 4px;
    padding: 12px;
    border: 1px solid color-mix(in srgb, var(--ok) 35%, var(--border));
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--ok) 7%, var(--panel));
  }
  .next-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .next-head .ok {
    display: inline-flex;
    padding: 3px;
    border-radius: 50%;
    background: var(--ok);
    color: #fff;
  }
  .next-head .close {
    margin-left: auto;
    width: 24px;
    height: 24px;
  }
  .next p {
    margin: 6px 0 10px;
    color: var(--text-2);
    font-size: 12.5px;
  }
  .next-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  /* Pictures: trace or sticker */
  .paths {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .way {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--panel);
    color: var(--text);
    text-align: left;
    cursor: pointer;
    transition: border-color 0.12s, background 0.12s;
  }
  .way:hover {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .way-icon {
    display: inline-flex;
    flex: none;
    padding: 9px;
    border-radius: 10px;
    background: var(--accent-soft);
    color: var(--accent);
  }
  .way > span:nth-child(2) {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 3px;
  }
  .way b {
    font-weight: 600;
  }
  .way small {
    color: var(--text-3);
    font-size: 11.5px;
    line-height: 1.35;
  }
  .arrow {
    display: inline-flex;
    color: var(--text-3);
  }
  .way:hover .arrow {
    color: var(--accent);
  }
  .link {
    align-self: flex-start;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-3);
    font-size: 12px;
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
  }
  .link:hover {
    color: var(--accent);
  }
  .pix {
    margin: 2px -16px -14px;
    border-top: 1px solid var(--border);
  }
  .pix :global(section) {
    border-bottom: 0;
  }
  .tools-head {
    padding: 16px 16px 6px;
    color: var(--text-3);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    border-bottom: 1px solid var(--border);
  }
  kbd {
    font: inherit;
    font-size: 11px;
    padding: 1px 5px;
    border: 1px solid var(--border-strong);
    border-bottom-width: 2px;
    border-radius: 4px;
    color: var(--text-2);
    background: var(--panel-2);
  }
</style>
