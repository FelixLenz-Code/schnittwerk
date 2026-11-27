<script lang="ts">
  // Classic menu bar: Datei, Bearbeiten, Objekt, Ansicht, Plotter, Hilfe.
  import { BOOL_OPS, combine, flip, putTextOnPath, selectedPaths, textOnPathPair } from "../actions";
  import { api } from "../api";
  import { customMat, editor, matKindOf, MATS, PAGES, rollMat, type PageName } from "../editor.svelte";
  import { fmt } from "../geometry";
  import { t } from "../i18n.svelte";
  import { copySelection, exportSvg, importDialog, newProject, openDialog, openProject, paste, recent, saveProject } from "../io.svelte";
  import { nodes } from "../nodes.svelte";
  import { plotter } from "../plotter.svelte";
  import { templatesUi } from "../templates.svelte";
  import { startTour } from "../tour.svelte";
  import { setTheme, ui } from "../ui.svelte";
  import { updater } from "../update.svelte";
  import { wizardUi } from "../wizard.svelte";
  import type { PathObj } from "../types";
  import Icon from "./Icon.svelte";

  type Item =
    | "sep"
    | {
        label: string;
        /** Already translated (file names, language names). */
        raw?: boolean;
        keys?: string;
        action?: () => void;
        disabled?: boolean;
        checked?: boolean;
        sub?: Item[];
      };
  interface Menu {
    id: string;
    label: string;
    /** Letter for Alt+letter (German and English labels share it where possible). */
    items: () => Item[];
  }

  const sel = () => editor.selected.length;
  /** A dialog covers the window – the menu stays closed. */
  const modalOpen = () =>
    plotter.dialogOpen || plotter.printOpen || templatesUi.open || wizardUi.open || updater.dialogOpen || ui.settingsOpen || ui.aboutOpen;
  const ctrl = () => t("Strg");

  async function quit() {
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().close();
    } catch {
      window.close();
    }
  }

  function toggleLock() {
    const objs = editor.selected;
    if (!objs.length) return;
    const lock = !objs.every((o) => o.locked);
    editor.checkpoint("Sperren");
    for (const o of objs) o.locked = lock;
    if (lock) editor.selection = [];
  }

  function pickMat(v: string) {
    if (editor.printCut.enabled) editor.setPrintCut(false);
    const kind = matKindOf(editor.mat);
    if (v === "roll") editor.mat = rollMat(kind === "roll" ? editor.mat.h : 600);
    else if (v === "custom") editor.mat = customMat(Math.max(editor.mat.w, 300), Math.max(editor.mat.h, 400));
    else editor.mat = MATS.find((m) => m.name === v) ?? MATS[0];
    editor.dirty = true;
    editor.fitMat();
  }

  function help(section: string | null) {
    editor.rightTab = "help";
    ui.helpSection = section;
  }

  const MENUS: Menu[] = [
    {
      id: "file",
      label: "Datei",
      items: () => [
        { label: "Startseite …", keys: `${ctrl()}+N`, action: newProject },
        { label: "Neu aus Vorlage …", keys: `${ctrl()}+⇧+N`, action: () => (templatesUi.open = true) },
        { label: "Öffnen …", keys: `${ctrl()}+O`, action: openDialog },
        {
          label: "Zuletzt geöffnet",
          disabled: !recent.list.length,
          sub: recent.list.slice(0, 8).map((r) => ({ label: r.name, raw: true, action: () => openProject(r.path) })),
        },
        "sep",
        { label: "Speichern", keys: `${ctrl()}+S`, action: () => saveProject() },
        { label: "Speichern unter …", keys: `${ctrl()}+⇧+S`, action: () => saveProject(true) },
        "sep",
        { label: "Importieren …", keys: `${ctrl()}+I`, action: importDialog },
        { label: "Als SVG exportieren …", keys: `${ctrl()}+E`, action: exportSvg },
        "sep",
        { label: "Drucken …", keys: `${ctrl()}+P`, action: () => (plotter.printOpen = true) },
        "sep",
        { label: "Einstellungen …", action: () => (ui.settingsOpen = true) },
        "sep",
        { label: "Beenden", keys: `${ctrl()}+Q`, action: quit },
      ],
    },
    {
      id: "edit",
      label: "Bearbeiten",
      items: () => [
        {
          label: editor.canUndo ? t("Rückgängig: {x}", { x: t(editor.undoLabel) }) : t("Rückgängig"),
          raw: true,
          keys: `${ctrl()}+Z`,
          action: () => editor.undo(),
          disabled: !editor.canUndo,
        },
        {
          label: editor.canRedo ? t("Wiederholen: {x}", { x: t(editor.redoLabel) }) : t("Wiederholen"),
          raw: true,
          keys: `${ctrl()}+Y`,
          action: () => editor.redo(),
          disabled: !editor.canRedo,
        },
        "sep",
        { label: "Ausschneiden", keys: `${ctrl()}+X`, action: () => copySelection(true), disabled: !sel() },
        { label: "Kopieren", keys: `${ctrl()}+C`, action: () => copySelection(), disabled: !sel() },
        { label: "Einfügen", keys: `${ctrl()}+V`, action: () => paste() },
        { label: "Duplizieren", keys: `${ctrl()}+D`, action: () => editor.duplicateSelected(), disabled: !sel() },
        { label: "Löschen", keys: t("Entf"), action: () => editor.removeSelected(), disabled: !sel() },
        "sep",
        { label: "Alles auswählen", keys: `${ctrl()}+A`, action: () => editor.selectAll() },
        { label: "Auswahl aufheben", keys: "Esc", action: () => (editor.selection = []), disabled: !sel() },
      ],
    },
    {
      id: "object",
      label: "Objekt",
      items: () => {
        const paths = selectedPaths();
        const single = editor.selected.length === 1 ? editor.selected[0] : null;
        return [
          { label: "Gruppieren", keys: `${ctrl()}+G`, action: () => editor.groupSelected(), disabled: sel() < 2 },
          { label: "Gruppierung lösen", keys: `${ctrl()}+⇧+G`, action: () => editor.ungroupSelected(), disabled: !editor.selected.some((o) => o.group) },
          "sep",
          { label: "Nach vorne", keys: `${ctrl()}+↑`, action: () => editor.restack(1), disabled: !sel() },
          { label: "Nach hinten", keys: `${ctrl()}+↓`, action: () => editor.restack(-1), disabled: !sel() },
          { label: "Spiegeln", action: () => flip("x"), disabled: !sel() },
          { label: "Kippen", action: () => flip("y"), disabled: !sel() },
          "sep",
          {
            label: "Ausrichten",
            disabled: !sel(),
            sub: [
              { label: "Links", action: () => editor.align("left") },
              { label: "Horizontal zentrieren", action: () => editor.align("hcenter") },
              { label: "Rechts", action: () => editor.align("right") },
              "sep",
              { label: "Oben", action: () => editor.align("top") },
              { label: "Vertikal zentrieren", action: () => editor.align("vcenter") },
              { label: "Unten", action: () => editor.align("bottom") },
            ],
          },
          {
            label: "Verteilen",
            disabled: editor.selectedUnits().length < 3,
            sub: [
              { label: "Waagrecht verteilen", action: () => editor.distribute("x") },
              { label: "Senkrecht verteilen", action: () => editor.distribute("y") },
            ],
          },
          {
            label: "Pfade kombinieren",
            disabled: paths.length < 2,
            sub: BOOL_OPS.map((b) => ({ label: b.label, action: () => combine(b.op) })),
          },
          "sep",
          {
            label: "Knoten bearbeiten",
            action: () => nodes.start(single as PathObj),
            disabled: !(single?.type === "path" && !single.locked),
          },
          { label: "Text auf Pfad", action: putTextOnPath, disabled: !textOnPathPair() },
          { label: "Sperren", action: toggleLock, disabled: !sel() },
        ];
      },
    },
    {
      id: "view",
      label: "Ansicht",
      items: () => {
        const kind = matKindOf(editor.mat);
        return [
          { label: "Vergrößern", keys: `${ctrl()}++`, action: () => editor.zoomBy(1.25) },
          { label: "Verkleinern", keys: `${ctrl()}+−`, action: () => editor.zoomBy(0.8) },
          { label: "Matte einpassen", keys: `${ctrl()}+0`, action: () => editor.fitMat() },
          { label: "Auswahl einpassen", keys: `${ctrl()}+2`, action: () => editor.fitSelection() },
          "sep",
          { label: "Raster anzeigen", checked: editor.showGrid, action: () => (editor.showGrid = !editor.showGrid) },
          { label: "Am Raster einrasten", checked: editor.gridSnap, action: () => (editor.gridSnap = !editor.gridSnap) },
          {
            label: "Rasterweite",
            sub: [0.5, 1, 2, 5, 10].map((g) => ({ label: `${fmt(g)} mm`, raw: true, checked: editor.gridStep === g, action: () => (editor.gridStep = g) })),
          },
          {
            label: "Winkelschritt",
            sub: [1, 5, 15, 45, 90].map((a) => ({ label: `${a}°`, raw: true, checked: editor.rotStep === a, action: () => (editor.rotStep = a) })),
          },
          { label: "An Hilfslinien und Objekten einrasten", checked: editor.snap, action: () => (editor.snap = !editor.snap) },
          {
            label: "Alle Hilfslinien entfernen",
            disabled: !editor.guides.v.length && !editor.guides.h.length,
            action: () => {
              editor.guides = { v: [], h: [] };
              editor.dirty = true;
              editor.rev++;
            },
          },
          "sep",
          {
            label: "Matte und Papier",
            sub: [
              ...MATS.map((m) => ({ label: m.name, checked: !editor.printCut.enabled && kind !== "roll" && kind !== "custom" && editor.mat.name === m.name, action: () => pickMat(m.name) })),
              "sep" as const,
              ...(Object.keys(PAGES) as PageName[]).map((p) => ({
                label: `${p} · Print & Cut`,
                raw: true,
                checked: editor.printCut.enabled && editor.printCut.page === p,
                action: () => editor.setPrintCut(true, p),
              })),
              "sep" as const,
              { label: "Rolle ohne Matte", checked: !editor.printCut.enabled && kind === "roll", action: () => pickMat("roll") },
              { label: "Eigene Größe (Kacheln)", checked: !editor.printCut.enabled && kind === "custom", action: () => pickMat("custom") },
            ],
          },
          "sep",
          {
            label: "Design",
            sub: [
              { label: "Wie das System", checked: ui.theme === "system", action: () => setTheme("system") },
              { label: "Hell", checked: ui.theme === "light", action: () => setTheme("light") },
              { label: "Dunkel", checked: ui.theme === "dark", action: () => setTheme("dark") },
            ],
          },
        ];
      },
    },
    {
      id: "plotter",
      label: "Plotter",
      items: () => [
        { label: "Schneiden …", keys: `${ctrl()}+Enter`, action: () => (plotter.dialogOpen = true) },
        { label: "Drucken …", keys: `${ctrl()}+P`, action: () => (plotter.printOpen = true) },
        "sep",
        { label: "Verbindung und Messer …", action: () => (ui.deviceMenuOpen = true) },
        {
          label: "Neues Messer eingesetzt",
          action: () => {
            plotter.resetBlade();
            editor.toast("Messer-Zähler zurückgesetzt", "ok");
          },
        },
      ],
    },
    {
      id: "help",
      label: "Hilfe",
      items: () => [
        { label: "Anleitung", keys: "F1", action: () => help(null) },
        { label: "Einführung starten", action: startTour },
        { label: "Tastenkürzel", action: () => help("keys") },
        "sep",
        { label: updater.available ? t("Update auf {v}", { v: updater.info?.latest ?? "" }) : t("Nach Updates suchen"), raw: true, action: () => (updater.available ? (updater.dialogOpen = true) : updater.check()) },
        { label: "Fehler melden oder Frage stellen", action: () => api.openUrl("https://github.com/FelixLenz-Code/schnittwerk/issues") },
        "sep",
        { label: "Über Schnittwerk", action: () => (ui.aboutOpen = true) },
      ],
    },
  ];

  let openId = $state<string | null>(null);
  let subOpen = $state<number | null>(null);
  let focus = $state(-1);
  let bar = $state<HTMLElement>();

  const current = $derived(openId ? MENUS.find((m) => m.id === openId) : null);
  const items = $derived(current ? current.items() : []);

  function openMenu(id: string | null, focusFirst = false) {
    openId = id;
    subOpen = null;
    focus = focusFirst && id ? nextIndex(-1, 1) : -1;
  }

  function nextIndex(from: number, dir: 1 | -1): number {
    const list = current ? current.items() : [];
    for (let k = 1; k <= list.length; k++) {
      const i = (from + dir * k + list.length * 2) % list.length;
      const it = list[i];
      if (it !== "sep" && !it.disabled) return i;
    }
    return -1;
  }

  function run(it: Item) {
    if (it === "sep" || it.disabled) return;
    if (it.sub) return;
    openMenu(null);
    it.action?.();
  }

  function label(it: Exclude<Item, "sep">) {
    return it.raw ? it.label : t(it.label);
  }

  function onKeydown(e: KeyboardEvent) {
    // F1 opens the guide.
    if (e.key === "F1") {
      e.preventDefault();
      help(null);
      return;
    }
    // Alt+letter opens a menu (Alt alone does not, it is used while dragging).
    if (e.altKey && !e.ctrlKey && !e.metaKey && e.key.length === 1 && !openId && !modalOpen()) {
      const k = e.key.toLowerCase();
      const m = MENUS.find((m) => t(m.label).toLowerCase().startsWith(k));
      if (m && !(e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement)) {
        e.preventDefault();
        e.stopPropagation();
        openMenu(m.id, true);
      }
      return;
    }
    if (!openId) return;
    const idx = MENUS.findIndex((m) => m.id === openId);
    const it = items[focus];
    const handled = (() => {
      switch (e.key) {
        case "Escape":
          if (subOpen !== null) subOpen = null;
          else openMenu(null);
          return true;
        case "ArrowDown":
          focus = nextIndex(focus, 1);
          return true;
        case "ArrowUp":
          focus = nextIndex(focus, -1);
          return true;
        case "ArrowRight":
          if (it && it !== "sep" && it.sub && !it.disabled) subOpen = focus;
          else openMenu(MENUS[(idx + 1) % MENUS.length].id, true);
          return true;
        case "ArrowLeft":
          if (subOpen !== null) subOpen = null;
          else openMenu(MENUS[(idx - 1 + MENUS.length) % MENUS.length].id, true);
          return true;
        case "Enter":
        case " ":
          if (it && it !== "sep") {
            if (it.sub && !it.disabled) subOpen = focus;
            else run(it);
          }
          return true;
      }
      return false;
    })();
    if (handled) {
      e.preventDefault();
      e.stopPropagation();
    }
  }
</script>

<svelte:window
  onkeydowncapture={onKeydown}
  onpointerdown={(e) => openId && !bar?.contains(e.target as Node) && openMenu(null)}
  onblur={() => openMenu(null)}
/>

<nav class="menubar" bind:this={bar} data-tour="menu" aria-label={t("Menü")}>
  {#each MENUS as m}
    <div class="top">
      <button
        class="title"
        class:open={openId === m.id}
        aria-haspopup="menu"
        aria-expanded={openId === m.id}
        onclick={() => openMenu(openId === m.id ? null : m.id)}
        onpointerenter={() => openId && openId !== m.id && openMenu(m.id)}><u>{t(m.label).charAt(0)}</u>{t(m.label).slice(1)}</button
      >
      {#if openId === m.id}
        <div class="drop" role="menu">
          {#each items as it, i}
            {#if it === "sep"}
              <div class="sep"></div>
            {:else}
              <div class="row-wrap" role="none" onpointerenter={() => { focus = i; subOpen = it.sub && !it.disabled ? i : null; }}>
                <button role="menuitem" class="item" class:focus={focus === i} disabled={it.disabled} onclick={() => (it.sub ? (subOpen = i) : run(it))}>
                  <span class="check">{#if it.checked}<Icon name="check" size={13} />{/if}</span>
                  <span class="lbl">{label(it)}</span>
                  {#if it.keys}<kbd>{it.keys}</kbd>{/if}
                  {#if it.sub}<span class="arrow"><Icon name="chevronLeft" size={12} /></span>{/if}
                </button>
                {#if it.sub && subOpen === i && !it.disabled}
                  <div class="drop sub" role="menu">
                    {#each it.sub as s}
                      {#if s === "sep"}
                        <div class="sep"></div>
                      {:else}
                        <button role="menuitem" class="item" disabled={s.disabled} onclick={() => run(s)}>
                          <span class="check">{#if s.checked}<Icon name="check" size={13} />{/if}</span>
                          <span class="lbl">{label(s)}</span>
                        </button>
                      {/if}
                    {/each}
                  </div>
                {/if}
              </div>
            {/if}
          {/each}
        </div>
      {/if}
    </div>
  {/each}
</nav>

<style>
  .menubar {
    display: flex;
    align-items: stretch;
    height: 28px;
    padding: 0 6px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    font-size: 12.5px;
    position: relative;
    z-index: 45;
  }
  .top {
    position: relative;
  }
  .title {
    height: 100%;
    padding: 0 10px;
    border: 0;
    background: none;
    color: var(--text-2);
    cursor: pointer;
    border-radius: var(--radius-sm);
  }
  .title u {
    text-decoration: none;
  }
  .menubar:focus-within .title u,
  .title.open u {
    text-decoration: underline;
  }
  .title:hover,
  .title.open {
    background: var(--panel-2);
    color: var(--text);
  }
  .drop {
    position: absolute;
    top: 100%;
    left: 0;
    min-width: 240px;
    padding: 5px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.25);
    display: flex;
    flex-direction: column;
  }
  .drop.sub {
    top: -6px;
    left: 100%;
    margin-left: 2px;
    min-width: 200px;
    max-width: 340px;
  }
  .row-wrap {
    position: relative;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    height: 27px;
    padding: 0 8px 0 4px;
    border: 0;
    border-radius: var(--radius-sm);
    background: none;
    text-align: left;
    cursor: pointer;
    color: var(--text);
  }
  .item:not(:disabled):hover,
  .item.focus:not(:disabled) {
    background: var(--panel-2);
  }
  .item:disabled {
    color: var(--text-3);
    cursor: default;
  }
  .check {
    width: 16px;
    display: inline-flex;
    justify-content: center;
    color: var(--accent);
  }
  .lbl {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  kbd {
    padding-left: 18px;
    font: inherit;
    font-size: 11px;
    color: var(--text-3);
  }
  .arrow {
    display: inline-flex;
    transform: rotate(180deg);
    color: var(--text-3);
  }
  .sep {
    height: 1px;
    margin: 4px 2px;
    background: var(--border);
  }
</style>
