<script lang="ts">
  import { onMount } from "svelte";
  import { inApp } from "./lib/api";
  import { editor } from "./lib/editor.svelte";
  import { fmt } from "./lib/geometry";
  import { t } from "./lib/i18n.svelte";
  import { confirmClose, copySelection, exportSvg, findRecovery, importDialog, importPaths, newProject, openDialog, paste, saveProject, startAutosave, type Recovery } from "./lib/io.svelte";
  import { wizardUi } from "./lib/wizard.svelte";
  import NewProjectWizard from "./lib/components/NewProjectWizard.svelte";
  import UpdateDialog from "./lib/components/UpdateDialog.svelte";
  import ContextMenu from "./lib/components/ContextMenu.svelte";
  import Tour from "./lib/components/Tour.svelte";
  import { startTour, tour } from "./lib/tour.svelte";
  import { updater } from "./lib/update.svelte";
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
  import MenuBar from "./lib/components/MenuBar.svelte";
  import SurfacePicker from "./lib/components/SurfacePicker.svelte";
  import MaterialChip from "./lib/components/MaterialChip.svelte";
  import SettingsDialog from "./lib/components/SettingsDialog.svelte";
  import AboutDialog from "./lib/components/AboutDialog.svelte";
  import MaterialsDialog from "./lib/components/MaterialsDialog.svelte";
  import { ui } from "./lib/ui.svelte";


  // ---------- window title ----------
  $effect(() => {
    const name = editor.filePath?.split("/").pop() ?? t("Unbenannt");
    document.title = `${editor.dirty ? "• " : ""}${name} – Schnittwerk`;
  });

  // ---------- drag & drop + startup ----------
  let dropping = $state(false);
  let recovery = $state<Recovery | null>(null);
  // The tour starts once at the very first start, after the assistant is closed.
  let startupChecked = $state(false);
  $effect(() => {
    if (startupChecked && !tour.done && tour.step < 0 && !wizardUi.open) setTimeout(startTour, 400);
  });
  onMount(() => {
    requestAnimationFrame(() => editor.fitMat());
    plotter.start();
    startAutosave();
    // Look for a new version a little after start (quietly, see update.svelte.ts).
    setTimeout(() => updater.autoCheck(), 4000);
    // Leftover unsaved work is offered in the assistant, which opens at
    // start unless switched off.
    findRecovery().then((r) => {
      if (r || wizardUi.atStart) wizardUi.open = true;
      startupChecked = true;
    });
    if (!inApp) {
      editor.toast(t("Browser-Vorschau: Dateizugriff und Tracen nur in der Desktop-App."), "info");
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
        const kind = ev.payload.type;
        if (kind === "enter" || kind === "over") dropping = true;
        else if (kind === "leave") dropping = false;
        else if (kind === "drop") {
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
    if (plotter.dialogOpen || plotter.printOpen || templatesUi.open || wizardUi.open || updater.dialogOpen || ui.settingsOpen || ui.aboutOpen || ui.materialsOpen) return;
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
        "2": () => editor.fitSelection(),
        q: () => import("@tauri-apps/api/window").then(({ getCurrentWindow }) => getCurrentWindow().close()),
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

</script>

<svelte:window onkeydown={onKeydown} />

<div class="app">
  <MenuBar />
  <header>
    <button class="brand" data-tour="home" title={t("Startseite: neues Projekt, zuletzt geöffnet")} onclick={newProject}>
      <span class="logo"><Icon name="scissors" size={16} stroke={2.2} /></span>
      <span class="title">Schnittwerk</span>
    </button>

    <div class="group">
      <button class="btn icon save" data-tour="save" class:dirty={editor.dirty} title={editor.dirty ? t("Speichern (Strg+S) – ungespeicherte Änderungen") : t("Speichern (Strg+S)")} onclick={() => saveProject()}><Icon name="save" /></button>
      <button class="btn outline" data-tour="import" title={t("Bild oder SVG importieren (Strg+I)")} onclick={importDialog}><Icon name="imagePlus" /> <span class="lbl">{t("Importieren")}</span></button>
    </div>
    <span class="divider"></span>
    <div class="group" data-tour="undo">
      <button class="btn icon" title={editor.canUndo ? t("Rückgängig: {x} (Strg+Z)", { x: t(editor.undoLabel) }) : t("Rückgängig (Strg+Z)")} disabled={!editor.canUndo} onclick={() => editor.undo()}><Icon name="undo" /></button>
      <button class="btn icon" title={editor.canRedo ? t("Wiederholen: {x} (Strg+Y)", { x: t(editor.redoLabel) }) : t("Wiederholen (Strg+Y)")} disabled={!editor.canRedo} onclick={() => editor.redo()}><Icon name="redo" /></button>
    </div>
    <div class="spacer"></div>
    {#if updater.available}
      <button class="btn update" title={t("Schnittwerk {v} ist verfügbar", { v: updater.info?.latest ?? "" })} onclick={() => (updater.dialogOpen = true)}><Icon name="download" /> Update</button>
    {/if}
    <DeviceMenu />
    <MaterialChip />
    {#if editor.printCut.enabled}
      <button class="btn outline" data-tour="print" title={t("Für Print & Cut drucken (Strg+P)")} onclick={() => (plotter.printOpen = true)}><Icon name="printer" /> {t("Drucken")}</button>
    {/if}
    <button class="btn primary" data-tour="cut" title={t("Schneiden-Dialog öffnen (Strg+Enter)")} onclick={() => (plotter.dialogOpen = true)}><Icon name="scissors" /> {t("Schneiden")}</button>
  </header>

  <main>
    <div class="rail" data-tour="tools">
      {#each TOOLS as tool}
        <button class="btn icon tool" class:active={editor.tool === tool.id} title={tool.key ? `${t(tool.label)} (${tool.key})` : t(tool.label)} onclick={() => (editor.tool = tool.id)}>
          <Icon name={tool.icon} size={18} />
        </button>
      {/each}
    </div>

    <div class="stage">
      <Canvas />
      <div class="surface"><SurfacePicker /></div>
      <div class="viewbar" data-tour="view">
        <button class="btn icon" class:active={editor.gridSnap} title={t("Am Raster einrasten ({g} mm, {a}°) – Alt beim Ziehen = frei. Rasterweite unter Ansicht.", { g: fmt(editor.gridStep), a: editor.rotStep })} onclick={() => (editor.gridSnap = !editor.gridSnap)}><Icon name="grid" /></button>
        <button class="btn icon" class:active={editor.snap} title={t("Einrasten an Hilfslinien, Matte und Objekten (Alt beim Ziehen = aus). Hilfslinien aus den Linealen ziehen, Rechtsklick löscht.")} onclick={() => (editor.snap = !editor.snap)}><Icon name="magnet" /></button>
        <span class="vdiv"></span>
        <button class="btn icon" title={t("Verkleinern")} onclick={() => editor.zoomBy(0.8)}><Icon name="zoomOut" /></button>
        <button class="btn zoom" title={t("Matte einpassen (Strg+0) · Auswahl einpassen (Strg+2)")} onclick={() => editor.fitMat()}>{Math.round((editor.view.zoom / 3.78) * 100)} %</button>
        <button class="btn icon" title={t("Vergrößern")} onclick={() => editor.zoomBy(1.25)}><Icon name="zoomIn" /></button>
      </div>
      {#if !editor.objects.length && !wizardUi.open && !dropping}
        <div class="empty">
          <div class="empty-card">
            <span class="empty-icon"><Icon name="imagePlus" size={26} stroke={1.6} /></span>
            <strong>{t("Bild hierher ziehen")}</strong>
            <span class="muted">{t("Foto, Zeichnung, Logo, SVG oder PDF – oder links eine Form zeichnen")}</span>
            <div class="empty-actions">
              <button class="btn primary" onclick={importDialog}><Icon name="imagePlus" /> {t("Importieren")}</button>
              <button class="btn outline" onclick={() => (templatesUi.open = true)}><Icon name="filePlus" /> {t("Aus Vorlage …")}</button>
            </div>
          </div>
        </div>
      {/if}
    </div>
    <RightPanel />

    {#if dropping}
      <div class="drop">
        <div class="drop-card"><Icon name="imagePlus" size={28} /> {t("Datei loslassen zum Importieren")}</div>
      </div>
    {/if}
  </main>

  <footer>
    <span class="mono">
      {#if editor.cursor}X {fmt(editor.cursor.x)} · Y {fmt(editor.cursor.y)} mm{:else}&nbsp;{/if}
    </span>
    <span>
      {#if editor.selected.length === 1}
        {t(editor.selected[0].name)} · {fmt(editor.selected[0].w)} × {fmt(editor.selected[0].h)} mm
      {:else if editor.selected.length > 1}
        {t("{n} Objekte ausgewählt", { n: editor.selected.length })}
      {/if}
    </span>
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
  <ContextMenu />
  {#if ui.settingsOpen}
    <SettingsDialog />
  {/if}
  {#if ui.aboutOpen}
    <AboutDialog />
  {/if}
  {#if ui.materialsOpen}
    <MaterialsDialog />
  {/if}
  <Tour />
  {#if updater.dialogOpen}
    <UpdateDialog />
  {/if}
  {#if wizardUi.open}
    <NewProjectWizard bind:recovery />
  {/if}

  <div class="toasts">
    {#each editor.toasts as toast (toast.id)}
      <div class="toast {toast.kind}">
        <Icon name={toast.kind === "error" ? "x" : toast.kind === "ok" ? "check" : "sparkle"} size={14} />
        {toast.text}
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
    margin-right: 6px;
    padding: 3px 10px 3px 3px;
    border: 0;
    border-radius: 10px;
    background: transparent;
    cursor: pointer;
  }
  .brand:hover {
    background: var(--panel-2);
  }
  /* Narrow windows: icon-only buttons (the title still names them). */
  @media (max-width: 1100px) {
    .lbl {
      display: none;
    }
  }
  .update {
    background: color-mix(in srgb, var(--ok) 15%, transparent);
    color: var(--ok);
    font-weight: 600;
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
  .save {
    position: relative;
  }
  .save.dirty::after {
    content: "";
    position: absolute;
    top: 5px;
    right: 5px;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
  }
  .stage {
    position: relative;
    flex: 1;
    display: flex;
    min-width: 0;
  }
  /* Floating controls on the canvas (below the 22 px rulers). */
  .surface {
    position: absolute;
    top: 32px;
    left: 32px;
    z-index: 5;
  }
  .viewbar {
    position: absolute;
    right: 14px;
    bottom: 14px;
    z-index: 5;
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 3px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--panel);
    box-shadow: var(--shadow);
  }
  .viewbar .btn {
    height: 28px;
  }
  .vdiv {
    width: 1px;
    height: 18px;
    margin: 0 3px;
    background: var(--border);
  }
  .zoom {
    min-width: 54px;
    justify-content: center;
    font-variant-numeric: tabular-nums;
    font-size: 12px;
  }
  .empty {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    pointer-events: none;
  }
  .empty-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    max-width: 320px;
    padding: 26px 28px 22px;
    border: 1.5px dashed var(--border-strong);
    border-radius: 16px;
    background: color-mix(in srgb, var(--panel) 92%, transparent);
    box-shadow: var(--shadow);
    text-align: center;
  }
  .empty-card strong {
    font-size: 15px;
  }
  .empty-card .muted {
    color: var(--text-3);
    font-size: 12.5px;
    line-height: 1.45;
  }
  .empty-icon {
    display: inline-flex;
    margin-bottom: 4px;
    padding: 12px;
    border-radius: 14px;
    background: var(--accent-soft);
    color: var(--accent);
  }
  .empty-actions {
    display: flex;
    gap: 8px;
    margin-top: 10px;
    pointer-events: auto;
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
    /* Above every dialog and the start page, so messages stay readable. */
    z-index: 300;
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
