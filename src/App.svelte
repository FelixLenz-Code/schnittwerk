<script lang="ts">
  import { onMount } from "svelte";
  import { inApp } from "./lib/api";
  import { customMat, editor, matKindOf, MATS, PAGES, rollMat, ROLL_MAX, type PageName } from "./lib/editor.svelte";
  import { fmt } from "./lib/geometry";
  import { confirmClose, copySelection, exportSvg, findRecovery, importDialog, importPaths, newProject, openDialog, paste, saveProject, startAutosave, type Recovery } from "./lib/io.svelte";
  import { wizardUi } from "./lib/wizard.svelte";
  import NewProjectWizard from "./lib/components/NewProjectWizard.svelte";
  import type { Tool } from "./lib/types";
  import Canvas from "./lib/components/Canvas.svelte";
  import Icon from "./lib/components/Icon.svelte";
  import RightPanel from "./lib/components/RightPanel.svelte";
  import CutDialog from "./lib/components/CutDialog.svelte";
  import PrintDialog from "./lib/components/PrintDialog.svelte";
  import DeviceMenu from "./lib/components/DeviceMenu.svelte";
  import { plotter } from "./lib/plotter.svelte";
  import { perspective } from "./lib/perspective.svelte";
  import { nodes } from "./lib/nodes.svelte";
  import { templatesUi } from "./lib/templates.svelte";
  import TemplatesDialog from "./lib/components/TemplatesDialog.svelte";

  // ---------- theme ----------
  type Theme = "system" | "light" | "dark";
  let theme = $state<Theme>(load("theme", "system") as Theme);
  function load(key: string, fallback: string) {
    try {
      return localStorage.getItem("schnittwerk." + key) ?? fallback;
    } catch {
      return fallback;
    }
  }
  $effect(() => {
    if (theme === "system") document.documentElement.removeAttribute("data-theme");
    else document.documentElement.setAttribute("data-theme", theme);
    try {
      localStorage.setItem("schnittwerk.theme", theme);
    } catch {}
  });
  const isDark = $derived(
    theme === "dark" || (theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches),
  );

  // ---------- window title ----------
  $effect(() => {
    const name = editor.filePath?.split("/").pop() ?? "Unbenannt";
    document.title = `${editor.dirty ? "• " : ""}${name} – Schnittwerk`;
  });

  // ---------- drag & drop + startup ----------
  let dropping = $state(false);
  let recovery = $state<Recovery | null>(null);
  onMount(() => {
    requestAnimationFrame(() => editor.fitMat());
    plotter.start();
    startAutosave();
    // Leftover unsaved work is offered in the assistant, which opens at
    // start unless switched off.
    findRecovery().then((r) => {
      if (r || wizardUi.atStart) wizardUi.open = true;
    });
    if (!inApp) {
      editor.toast("Browser-Vorschau: Dateizugriff und Tracen nur in der Desktop-App.", "info");
      return;
    }
    let unlisten: (() => void) | undefined;
    let unlistenClose: (() => void) | undefined;
    import("@tauri-apps/api/window").then(async ({ getCurrentWindow }) => {
      let asking = false;
      unlistenClose = await getCurrentWindow().onCloseRequested(async (ev) => {
        if (asking) return ev.preventDefault();
        asking = true;
        try {
          if (!(await confirmClose())) ev.preventDefault();
        } finally {
          asking = false;
        }
      });
    });
    import("@tauri-apps/api/webview").then(async ({ getCurrentWebview }) => {
      unlisten = await getCurrentWebview().onDragDropEvent((ev) => {
        const t = ev.payload.type;
        if (t === "enter" || t === "over") dropping = true;
        else if (t === "leave") dropping = false;
        else if (t === "drop") {
          dropping = false;
          importPaths(ev.payload.paths);
        }
      });
    });
    return () => {
      unlisten?.();
      unlistenClose?.();
    };
  });

  // The recovery copy may have changed (saved, discarded): look again
  // whenever the assistant opens.
  $effect(() => {
    if (!wizardUi.open) return;
    let live = true;
    findRecovery().then((r) => live && (recovery = r));
    return () => (live = false);
  });

  // ---------- keyboard ----------
  const TOOL_KEYS: Record<string, Tool> = { v: "select", h: "pan", r: "rect", e: "ellipse", p: "polygon", s: "star", l: "line", f: "pen", t: "text" };

  function onKeydown(e: KeyboardEvent) {
    if (plotter.dialogOpen || plotter.printOpen || templatesUi.open || wizardUi.open) return;
    const t = e.target as HTMLElement;
    if (t instanceof HTMLInputElement || t instanceof HTMLSelectElement || t instanceof HTMLTextAreaElement) return;
    const mod = e.ctrlKey || e.metaKey;
    const k = e.key.toLowerCase();
    const step = e.shiftKey ? 10 : e.altKey ? 0.1 : 1;

    if (nodes.objId) {
      const act: Record<string, () => void> = {
        delete: () => nodes.deleteSelected(),
        backspace: () => nodes.deleteSelected(),
        escape: () => nodes.stop(),
        enter: () => nodes.stop(),
        arrowleft: () => nodes.nudge(-step, 0),
        arrowright: () => nodes.nudge(step, 0),
        arrowup: () => nodes.nudge(0, -step),
        arrowdown: () => nodes.nudge(0, step),
      };
      if (act[k] && !mod) {
        act[k]();
        e.preventDefault();
        return;
      }
      if (mod && k === "a") {
        nodes.selectAll();
        e.preventDefault();
        return;
      }
    }
    if (mod) {
      const actions: Record<string, () => void> = {
        z: () => (e.shiftKey ? editor.redo() : editor.undo()),
        y: () => editor.redo(),
        d: () => editor.duplicateSelected(),
        c: () => copySelection(),
        x: () => copySelection(true),
        v: () => paste(),
        a: () => editor.selectAll(),
        i: () => importDialog(),
        o: () => openDialog(),
        s: () => saveProject(e.shiftKey),
        n: () => (e.shiftKey ? (templatesUi.open = true) : newProject()),
        e: () => exportSvg(),
        enter: () => (plotter.dialogOpen = true),
        p: () => (plotter.printOpen = true),
        g: () => (e.shiftKey ? editor.ungroupSelected() : editor.groupSelected()),
        "0": () => editor.fitMat(),
        "+": () => editor.zoomBy(1.25),
        "=": () => editor.zoomBy(1.25),
        "-": () => editor.zoomBy(0.8),
        arrowup: () => editor.restack(1),
        arrowdown: () => editor.restack(-1),
      };
      const fn = actions[k];
      if (fn) {
        e.preventDefault();
        fn();
      }
      return;
    }
    if (perspective.objId && k === "enter") {
      perspective.apply();
      e.preventDefault();
      return;
    }
    switch (k) {
      case "delete":
      case "backspace":
        editor.removeSelected();
        break;
      case "escape":
        if (perspective.objId) perspective.cancel();
        else if (editor.picking) editor.picking = null;
        else editor.selection = [];
        editor.tool = "select";
        break;
      case "arrowleft":
        editor.nudge(-step, 0);
        break;
      case "arrowright":
        editor.nudge(step, 0);
        break;
      case "arrowup":
        editor.nudge(0, -step);
        break;
      case "arrowdown":
        editor.nudge(0, step);
        break;
      case "+":
        editor.zoomBy(1.25);
        break;
      case "-":
        editor.zoomBy(0.8);
        break;
      default:
        if (TOOL_KEYS[k]) editor.tool = TOOL_KEYS[k];
        else return;
    }
    e.preventDefault();
  }

  const TOOLS: { id: Tool; icon: string; label: string; key: string }[] = [
    { id: "select", icon: "pointer", label: "Auswählen", key: "V" },
    { id: "pan", icon: "hand", label: "Ansicht verschieben", key: "H" },
    { id: "rect", icon: "square", label: "Rechteck (Umschalt = Quadrat)", key: "R" },
    { id: "ellipse", icon: "circle", label: "Ellipse (Umschalt = Kreis)", key: "E" },
    { id: "polygon", icon: "hexagon", label: "Vieleck", key: "P" },
    { id: "star", icon: "star", label: "Stern", key: "S" },
    { id: "heart", icon: "heart", label: "Herz", key: "" },
    { id: "line", icon: "line", label: "Linie (Umschalt = 15°-Schritte)", key: "L" },
    { id: "pen", icon: "pen", label: "Freihand", key: "F" },
    { id: "text", icon: "type", label: "Text", key: "T" },
  ];

  // ---------- work area ----------
  const matKind = $derived(matKindOf(editor.mat));
  function pickMat(v: string) {
    if (v === "roll") editor.mat = rollMat(matKind === "roll" ? editor.mat.h : 600);
    else if (v === "custom") editor.mat = customMat(Math.max(editor.mat.w, 300), Math.max(editor.mat.h, 400));
    else editor.mat = MATS.find((m) => m.name === v) ?? MATS[0];
    editor.fitMat();
  }
  function setMatSize(key: "w" | "h", raw: string) {
    const v = parseFloat(raw.replace(",", "."));
    if (!Number.isFinite(v)) return;
    editor.mat = matKind === "roll" ? rollMat(v) : customMat(key === "w" ? v : editor.mat.w, key === "h" ? v : editor.mat.h, editor.mat.roll);
    editor.dirty = true;
    editor.fitMat();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="app">
  <header>
    <div class="brand">
      <span class="logo"><Icon name="scissors" size={16} stroke={2.2} /></span>
      <span class="title">Schnittwerk</span>
    </div>

    <div class="group">
      <button class="btn icon" title="Neu (Strg+N)" onclick={newProject}><Icon name="filePlus" /></button>
      <button class="btn icon" title="Neu aus Vorlage (Strg+Umschalt+N)" onclick={() => (templatesUi.open = true)}><Icon name="layers" /></button>
      <button class="btn icon" title="Öffnen (Strg+O)" onclick={openDialog}><Icon name="folder" /></button>
      <button class="btn icon" title="Speichern (Strg+S)" onclick={() => saveProject()}><Icon name="save" /></button>
    </div>
    <span class="divider"></span>
    <div class="group">
      <button class="btn outline" title="Bild oder SVG importieren (Strg+I)" onclick={importDialog}><Icon name="imagePlus" /> Importieren</button>
      <button class="btn" title="Schnittlinien als SVG exportieren (Strg+E)" onclick={exportSvg}><Icon name="download" /> SVG</button>
    </div>
    <span class="divider"></span>
    <div class="group">
      <button class="btn icon" title="Rückgängig (Strg+Z)" disabled={!editor.canUndo} onclick={() => editor.undo()}><Icon name="undo" /></button>
      <button class="btn icon" title="Wiederholen (Strg+Y)" disabled={!editor.canRedo} onclick={() => editor.redo()}><Icon name="redo" /></button>
    </div>

    <div class="spacer"></div>

    <div class="group">
      <button class="btn icon" title="Verkleinern" onclick={() => editor.zoomBy(0.8)}><Icon name="zoomOut" /></button>
      <button class="btn zoom" title="Matte einpassen (Strg+0)" onclick={() => editor.fitMat()}>{Math.round((editor.view.zoom / 3.78) * 100)} %</button>
      <button class="btn icon" title="Vergrößern" onclick={() => editor.zoomBy(1.25)}><Icon name="zoomIn" /></button>
      <button class="btn icon" class:active={editor.showGrid} title="Raster" onclick={() => (editor.showGrid = !editor.showGrid)}><Icon name="grid" /></button>
      <button class="btn icon" class:active={editor.snap} title="Einrasten an Hilfslinien, Matte und Objekten (Alt beim Ziehen = aus). Hilfslinien aus den Linealen ziehen, Rechtsklick löscht." onclick={() => (editor.snap = !editor.snap)}><Icon name="magnet" /></button>
      <button class="btn icon" title="Design wechseln" onclick={() => (theme = isDark ? "light" : "dark")}><Icon name={isDark ? "sun" : "moon"} /></button>
    </div>
    <span class="divider"></span>
    <DeviceMenu />
    <button class="btn outline" title="Für Print & Cut drucken (Strg+P)" onclick={() => (plotter.printOpen = true)}><Icon name="printer" /> Drucken</button>
    <button class="btn primary" title="Schneiden-Dialog öffnen (Strg+Enter)" onclick={() => (plotter.dialogOpen = true)}><Icon name="scissors" /> Schneiden</button>
  </header>

  <main>
    <div class="rail">
      {#each TOOLS as t}
        <button class="btn icon tool" class:active={editor.tool === t.id} title={t.key ? `${t.label} (${t.key})` : t.label} onclick={() => (editor.tool = t.id)}>
          <Icon name={t.icon} size={18} />
        </button>
      {/each}
    </div>

    <Canvas />
    <RightPanel />

    {#if dropping}
      <div class="drop">
        <div class="drop-card"><Icon name="imagePlus" size={28} /> Datei loslassen zum Importieren</div>
      </div>
    {/if}
  </main>

  <footer>
    <span class="mono">
      {#if editor.cursor}X {fmt(editor.cursor.x)} · Y {fmt(editor.cursor.y)} mm{:else}&nbsp;{/if}
    </span>
    <span>
      {#if editor.selected.length === 1}
        {editor.selected[0].name} · {fmt(editor.selected[0].w)} × {fmt(editor.selected[0].h)} mm
      {:else if editor.selected.length > 1}
        {editor.selected.length} Objekte ausgewählt
      {/if}
    </span>
    <span class="spacer"></span>
    <label class="pc" title="Seite mit Passermarken für Print & Cut">
      <input type="checkbox" checked={editor.printCut.enabled} onchange={(e) => editor.setPrintCut(e.currentTarget.checked)} />
      Print & Cut
    </label>
    {#if editor.printCut.enabled}
      <label class="mat">
        Seite
        <select value={editor.printCut.page} onchange={(e) => editor.setPrintCut(true, e.currentTarget.value as PageName)}>
          {#each Object.keys(PAGES) as p}<option value={p}>{p}</option>{/each}
        </select>
      </label>
    {:else}
      <label class="mat">
        Fläche
        <select value={matKind === "roll" ? "roll" : matKind === "custom" ? "custom" : editor.mat.name} onchange={(e) => pickMat(e.currentTarget.value)}>
          {#each MATS as m}<option value={m.name}>{m.name}</option>{/each}
          <option value="roll">Rolle ohne Matte</option>
          <option value="custom">Eigene Größe (Kacheln)</option>
        </select>
      </label>
      {#if matKind === "roll"}
        <label class="mat" title="Länge der Folie, die geschnitten wird">
          Länge
          <input class="size" type="number" min="50" max={ROLL_MAX} step="10" value={Math.round(editor.mat.h)} onchange={(e) => setMatSize("h", e.currentTarget.value)} /> mm
        </label>
      {:else if matKind === "custom"}
        <label class="mat">
          <input class="size" type="number" min="20" step="10" value={Math.round(editor.mat.w)} onchange={(e) => setMatSize("w", e.currentTarget.value)} />
          ×
          <input class="size" type="number" min="20" step="10" value={Math.round(editor.mat.h)} onchange={(e) => setMatSize("h", e.currentTarget.value)} /> mm
        </label>
      {/if}
    {/if}
  </footer>

  {#if plotter.dialogOpen}
    <CutDialog />
  {/if}
  {#if plotter.printOpen}
    <PrintDialog />
  {/if}
  {#if templatesUi.open}
    <TemplatesDialog />
  {/if}
  {#if wizardUi.open}
    <NewProjectWizard bind:recovery />
  {/if}

  <div class="toasts">
    {#each editor.toasts as t (t.id)}
      <div class="toast {t.kind}">
        <Icon name={t.kind === "error" ? "x" : t.kind === "ok" ? "check" : "sparkle"} size={14} />
        {t.text}
      </div>
    {/each}
  </div>
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100%;
  }
  header {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 48px;
    padding: 0 10px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    padding-right: 10px;
  }
  .logo {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: 8px;
    background: linear-gradient(135deg, var(--accent), #ff7eb9);
    color: #fff;
  }
  .title {
    font-weight: 700;
    letter-spacing: -0.01em;
    font-size: 14px;
  }
  .group {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .divider {
    width: 1px;
    height: 22px;
    background: var(--border);
    margin: 0 6px;
  }
  .spacer {
    flex: 1;
  }
  .zoom {
    min-width: 58px;
    justify-content: center;
    font-variant-numeric: tabular-nums;
  }
  main {
    position: relative;
    flex: 1;
    display: flex;
    min-height: 0;
  }
  .rail {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 8px 6px;
    background: var(--panel);
    border-right: 1px solid var(--border);
  }
  .tool {
    width: 36px;
    height: 36px;
  }
  footer {
    display: flex;
    align-items: center;
    gap: 16px;
    height: 28px;
    padding: 0 12px;
    background: var(--panel);
    border-top: 1px solid var(--border);
    color: var(--text-2);
    font-size: 12px;
  }
  .mono {
    font-variant-numeric: tabular-nums;
    min-width: 150px;
  }
  .mat {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .pc {
    display: flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
  }
  .pc input {
    accent-color: var(--accent);
  }
  .size {
    width: 64px;
    height: 20px;
    padding: 0 4px;
    border: 1px solid var(--border);
    border-radius: 5px;
    background: var(--panel-2);
    font-size: 12px;
    text-align: right;
  }
  .mat select {
    border: 0;
    background: transparent;
    color: var(--text);
    cursor: pointer;
  }
  .drop {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    border: 2px dashed var(--accent);
    pointer-events: none;
  }
  .drop-card {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 16px 22px;
    border-radius: var(--radius);
    background: var(--panel);
    box-shadow: var(--shadow);
    font-weight: 600;
    color: var(--accent);
  }
  .toasts {
    position: fixed;
    bottom: 40px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: center;
    pointer-events: none;
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 9px 14px;
    border-radius: var(--radius);
    background: var(--text);
    color: var(--bg);
    box-shadow: var(--shadow);
    animation: rise 0.18s ease-out;
    max-width: 520px;
  }
  .toast.error {
    background: var(--danger);
    color: #fff;
  }
  .toast.ok :global(svg) {
    color: var(--ok);
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }
</style>
