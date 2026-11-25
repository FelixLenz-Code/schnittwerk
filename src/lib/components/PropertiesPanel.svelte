<script lang="ts">
  import { api } from "../api";
  import { editor } from "../editor.svelte";
  import { aabb, fmt, uid } from "../geometry";
  import { pathMatrix } from "../plotter.svelte";
  import type { Obj, PathObj } from "../types";
  import Icon from "./Icon.svelte";
  import Toggle from "./Toggle.svelte";
  import TextEditor from "./TextEditor.svelte";
  import { nodes } from "../nodes.svelte";
  import { pathBbox, parsePath } from "../pathdata";
  import { polysBox, weedLines, worldPolys } from "../weed";
  import { refreshShape, rememberSpec, SHAPE_NAMES } from "../shapes";
  import Slider from "./Slider.svelte";
  import { t } from "../i18n.svelte";
  import { BOOL_OPS, combine, CUT_TYPES, flip, placeUnit, putTextOnPath, setCutColor, sheetArea, textOnPathPair, unitOutline } from "../actions";
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

  function frameSelection() {
    const b = aabb(editor.selected);
    if (!b) return;
    const pad = 2, r = 2;
    const w = b.x1 - b.x0 + 2 * pad, h = b.y1 - b.y0 + 2 * pad;
    const d = `M${r} 0H${w - r}A${r} ${r} 0 0 1 ${w} ${r}V${h - r}A${r} ${r} 0 0 1 ${w - r} ${h}H${r}A${r} ${r} 0 0 1 0 ${h - r}V${r}A${r} ${r} 0 0 1 ${r} 0Z`;
    const frame: PathObj = {
      id: uid(), type: "path", name: t("Schnittrahmen"), d, bbox: [0, 0, w, h], color: "#3e63dd", fill: false,
      x: b.x0 - pad, y: b.y0 - pad, w, h, rot: 0, flipX: false, flipY: false, visible: true, locked: false,
    };
    editor.add([frame], false);
    editor.toast("Rahmen zum Durchschneiden hinzugefügt", "ok");
  }

  // ---------- weeding aids ----------
  let weedMargin = $state(3);
  let weedSpacing = $state(20);
  let weedDir = $state<"h" | "v" | "hv" | "none">("h");

  /** Box around the design plus lines through the waste (vinyl). */
  function addWeeding() {
    const designs = selPaths.filter((o) => o.visible && o.cut !== false);
    if (!designs.length) return;
    const polys = worldPolys(designs);
    const b0 = polysBox(polys);
    if (!b0) return;
    const m = Math.max(0.5, weedMargin);
    const box = { x0: b0.x0 - m, y0: b0.y0 - m, x1: b0.x1 + m, y1: b0.y1 + m };
    const color = designs[0].color;
    const group = uid();
    const mk = (name: string, d: string): PathObj | null => {
      if (!d) return null;
      const [x, y, w, h] = pathBbox(parsePath(d));
      return {
        id: uid(), type: "path", name, d, bbox: [x, y, w || 0.01, h || 0.01], color, fill: false,
        x, y, w: w || 0.01, h: h || 0.01, rot: 0, flipX: false, flipY: false, visible: true, locked: false, group,
      };
    };
    const rect = `M${box.x0} ${box.y0}H${box.x1}V${box.y1}H${box.x0}Z`;
    const lines =
      weedDir === "none"
        ? ""
        : weedLines(polys, box, Math.max(3, weedSpacing), Math.min(m, Math.max(0.5, weedMargin / 2)), {
            horizontal: weedDir.includes("h"),
            vertical: weedDir.includes("v"),
          });
    const objs = [mk(t("Entgitterungsrahmen"), rect), mk(t("Entgitterungslinien"), lines)].filter((o): o is PathObj => !!o);
    editor.add(objs, false);
    editor.toast(lines ? "Entgitterungsrahmen und -linien angelegt" : "Entgitterungsrahmen angelegt", "ok");
  }

  let boolBusy = $state(false);
  async function runCombine(op: (typeof BOOL_OPS)[number]["op"]) {
    boolBusy = true;
    await combine(op);
    boolBusy = false;
  }
  const textOnPath = $derived(textOnPathPair());

  /** Selected objects split into units (a group or a single object). */
  function selectedUnits(): Obj[][] {
    const ids = editor.expandGroups(editor.selection);
    const byKey = new Map<string, Obj[]>();
    for (const o of editor.objects) {
      if (!ids.includes(o.id) || o.locked) continue;
      const k = o.group ?? o.id;
      byKey.set(k, [...(byKey.get(k) ?? []), o]);
    }
    return [...byKey.values()];
  }

  // ---------- sheet fill / nesting ----------
  let gap = $state(3);
  let nestShapes = $state(true);
  let nesting = $state(false);

  async function nestSelection(fill: boolean) {
    const units = selectedUnits();
    if (!units.length) return;
    if (fill && !nestShapes) return gridFill();
    nesting = true;
    try {
      const { area, keepouts } = sheetArea();
      const items = (fill ? units.slice(0, 1) : units).map((u) => ({ paths: unitOutline(u) }));
      if (fill && units.length > 1) {
        // Fill treats the whole selection as one unit.
        items[0] = { paths: units.flatMap(unitOutline) };
      }
      const r = await api.nest({ items, area, keepouts, gap, rotate: true, fill });
      if (!r.placements.length) {
        editor.toast("Passt nicht auf den Bogen.", "error");
        return;
      }
      editor.checkpoint("Bogen füllen");
      if (fill) {
        const unit = units.flat();
        r.placements.forEach((p, i) => {
          let objs = unit;
          if (i > 0) {
            const groupMap = new Map<string, string>();
            objs = unit.map((o) => {
              const c = { ...($state.snapshot(o) as Obj), id: uid() };
              if (c.group) c.group = groupMap.get(c.group) ?? groupMap.set(c.group, uid()).get(c.group);
              return c;
            });
            // Copies start from the original pose.
            editor.objects.push(...objs);
            objs = editor.objects.slice(-objs.length);
          }
          if (i === 0) {
            // Move the original last so copies can be cloned from its start pose.
            return;
          }
          placeUnit(objs, p.rot, p.center, p.dx, p.dy);
        });
        placeUnit(unit, r.placements[0].rot, r.placements[0].center, r.placements[0].dx, r.placements[0].dy);
        editor.toast(t("{n} Exemplare platzsparend verteilt", { n: r.placements.length }), "ok");
      } else {
        for (const p of r.placements) placeUnit(units[p.item], p.rot, p.center, p.dx, p.dy);
        editor.toast(
          r.unplaced.length
            ? t("{a} angeordnet, {b} passen nicht mehr drauf", { a: r.placements.length, b: r.unplaced.length })
            : t("{n} Teile platzsparend angeordnet", { n: r.placements.length }),
          r.unplaced.length ? "error" : "ok",
        );
      }
    } catch (e) {
      editor.error(e);
    } finally {
      nesting = false;
    }
  }

  /** Simple grid fill by bounding box (no rotation). */
  function gridFill() {
    const unit = editor.selected.filter((o) => !o.locked);
    const b = aabb(unit);
    if (!b) return;
    const bw = b.x1 - b.x0, bh = b.y1 - b.y0;
    const r = editor.regmarks;
    const area = r
      ? { x0: r.originX + 1, y0: r.originY + 1, x1: r.originX + r.width - 1, y1: r.originY + r.length - 1 }
      : { x0: 5, y0: 5, x1: editor.mat.w - 5, y1: editor.mat.h - 5 };
    const zones = editor.regZones();
    const hits = (x: number, y: number) =>
      zones.some(([zx, zy, zw, zh]) => x < zx + zw + 1 && x + bw > zx - 1 && y < zy + zh + 1 && y + bh > zy - 1);
    const cells: [number, number][] = [];
    for (let y = area.y0; y + bh <= area.y1; y += bh + gap) {
      for (let x = area.x0; x + bw <= area.x1; x += bw + gap) {
        if (!hits(x, y)) cells.push([x, y]);
      }
    }
    if (!cells.length) {
      editor.toast("Motiv ist zu groß für den Bogen.", "error");
      return;
    }
    editor.checkpoint("Bogen füllen");
    const [fx, fy] = cells[0];
    for (const o of unit) {
      o.x += fx - b.x0;
      o.y += fy - b.y0;
    }
    const copies: Obj[] = [];
    for (const [cx, cy] of cells.slice(1)) {
      const groupMap = new Map<string, string>();
      for (const o of unit) {
        const c = { ...($state.snapshot(o) as Obj), id: uid(), x: o.x + cx - fx, y: o.y + cy - fy };
        if (c.group) c.group = groupMap.get(c.group) ?? groupMap.set(c.group, uid()).get(c.group);
        copies.push(c);
      }
    }
    editor.objects.push(...copies);
    editor.toast(t("{n} Exemplare auf dem Bogen", { n: cells.length }), "ok");
  }

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

{#if !editor.selected.length}
  <div class="empty">
    <p><strong>{t("Nichts ausgewählt")}</strong></p>
    <p class="muted">{t("Importiere eine Datei")} (<kbd>{t("Strg")}</kbd>+<kbd>I</kbd>) {t("oder ziehe sie direkt auf die Arbeitsfläche.")}</p>
    <div class="tips">
      <div><kbd>V</kbd> {t("Auswahl")}</div>
      <div><kbd>H</kbd> / <kbd>{t("Leertaste")}</kbd> {t("Verschieben")}</div>
      <div><kbd>R</kbd> {t("Rechteck")} · <kbd>E</kbd> {t("Ellipse")} · <kbd>P</kbd> {t("Vieleck")} · <kbd>S</kbd> {t("Stern")}</div>
      <div><kbd>L</kbd> {t("Linie")} · <kbd>F</kbd> {t("Freihand")} · <kbd>T</kbd> {t("Text")}</div>
      <div>{t("Hilfslinien aus den Linealen ziehen")}</div>
      <div>{t("Mausrad = Zoom")} · <kbd>{t("Strg")}</kbd>+<kbd>0</kbd> {t("Einpassen")}</div>
    </div>
  </div>
{:else}
  {#if single}
    <section>
      <div class="title-row">
        <Icon name={single.type === "image" ? "imagePlus" : "scissors"} />
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

    <Section id="pos" title={t("Position & Größe")}>
      <div class="grid">
        {#each [["x", "X"], ["y", "Y"], ["w", "B"], ["h", "H"]] as [key, label]}
          <label class="num">
            <span>{t(label)}</span>
            <input
              class="field"
              value={fmt(single[key as "x"], 2)}
              disabled={single.locked}
              onchange={(e) => setField(single, key as "x", e.currentTarget.value)}
            />
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

    {#if single.type === "path" && single.shape && single.shape.kind !== "ellipse" && single.shape.kind !== "heart"}
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

    {#if single.type === "path"}
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

    {#if single.type === "path" && single.text}
      <TextEditor obj={single as PathObj & { text: NonNullable<PathObj["text"]> }} />
    {/if}

    {#if single.type === "path"}
      <Section id="line" title={t("Schnittlinie")}>
        <div class="colors">
          {#each CUT_COLORS as c}
            <button class="color" class:on={single.color === c} style:background={c} aria-label={t("Farbe {c}", { c })} onclick={() => setColor(single, c)}></button>
          {/each}
          <input type="color" value={single.color} onchange={(e) => setColor(single, e.currentTarget.value)} title={t("Eigene Farbe")} />
        </div>
        <div class="stack">
          <Toggle
            label="Fläche anzeigen"
            bind:checked={single.fill}
            hint="Nur Anzeige – geschnitten wird immer die Linie"
          />
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
      </Section>
    {:else}
      <Section id="image" title={t("Bild")}>
        <div class="stack">
          <label class="range-row">
            <span>{t("Deckkraft")}</span>
            <input type="range" min="0.1" max="1" step="0.05" bind:value={single.opacity} />
          </label>
          <button class="btn primary" onclick={() => (editor.rightTab = "trace")}><Icon name="wand" /> {t("Bild tracen …")}</button>
        </div>
      </Section>
    {/if}
  {:else if box}
    <section>
      <h3 class="section-title">{t("{n} Objekte", { n: editor.selected.length })}</h3>
      <p class="muted">{fmt(box.x1 - box.x0)} × {fmt(box.y1 - box.y0)} mm</p>
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
    </section>
  {/if}

  {#if selPaths.length}
    <Section id="cuttype" title={t("Schnittart")}>
      <div class="cut-types">
        {#each CUT_TYPES as ct}
          <button
            class="btn outline ctype"
            class:active={selPaths.every((o) => o.color.toLowerCase() === ct.color)}
            title={t(ct.hint)}
            onclick={() => setCutColor(ct.color)}
          >
            <span class="dot" style:background={ct.color}></span>{t(ct.label)}
          </button>
        {/each}
      </div>
      <p class="muted small">{t("Jede Farbe bekommt beim Schneiden eigene Einstellungen – angeschnitten wird zuerst.")}</p>
    </Section>
  {/if}

  {#if selPaths.length >= 2}
    <Section id="bool" title={t("Pfade kombinieren")} open={false}>
      <div class="cut-types">
        {#each BOOL_OPS as b}
          <button class="btn outline" disabled={boolBusy} title={t(b.hint)} onclick={() => runCombine(b.op)}>{t(b.label)}</button>
        {/each}
      </div>
    </Section>
  {/if}

  <Section id="frame" title={t("Rahmen")} open={false}>
    <button class="btn outline" onclick={frameSelection} title={t("Abgerundeter Rahmen mit 2 mm Abstand, wird durchgeschnitten")}>{t("Schnittrahmen um Auswahl")}</button>
  </Section>

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
        <button class="btn outline" onclick={addWeeding} title={t("Rahmen um das Motiv und Linien durch den Abfall – die überschüssige Folie lässt sich in Streifen abziehen")}>{t("Entgitterungshilfe anlegen")}</button>
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

  <Section id="fill" title={t("Bogen füllen")} open={false}>
    <div class="row">
      <label class="num grow-field">
        <span>{t("Abstand")}</span>
        <input class="field pad-wide" type="number" min="0" step="0.5" bind:value={gap} />
        <em>mm</em>
      </label>
      <button class="btn outline" disabled={nesting} onclick={() => nestSelection(true)} title={t("Bogen mit Kopien der Auswahl füllen")}>{t("Füllen")}</button>
      <button class="btn outline" disabled={nesting} onclick={() => nestSelection(false)} title={t("Ausgewählte Teile platzsparend anordnen (ohne Kopien)")}>{t("Anordnen")}</button>
    </div>
    <div class="stack">
      <Toggle label="Formen verschachteln" bind:checked={nestShapes} hint="Echte Umrisse ineinander setzen und drehen statt eines starren Rasters" />
    </div>
    <p class="muted small">{editor.printCut.enabled ? t("Verteilt die Auswahl so oft wie möglich auf der Seite (ohne Markenzonen).") : t("Verteilt die Auswahl so oft wie möglich auf der Matte.")}</p>
  </Section>

  <Section id="align" title={editor.selected.length === 1 ? t("Ausrichten auf Matte") : t("Ausrichten")}>
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
  .empty {
    padding: 36px 24px;
    color: var(--text-2);
  }
  .empty p {
    margin: 6px 0;
  }
  .tips {
    margin-top: 20px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    color: var(--text-3);
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
