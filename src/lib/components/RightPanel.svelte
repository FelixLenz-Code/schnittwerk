<script lang="ts">
  // Side panel that follows the selection: the sheet when nothing is
  // selected, the object otherwise; pictures lead on to tracing or the
  // sticker assistant. Layers sit in a collapsible area at the bottom.
  import { editor } from "../editor.svelte";
  import Icon from "./Icon.svelte";
  import { t } from "../i18n.svelte";
  import LayersPanel from "./LayersPanel.svelte";
  import PagePanel from "./PagePanel.svelte";
  import PropertiesPanel from "./PropertiesPanel.svelte";
  import TracePanel from "./TracePanel.svelte";
  import StickerPanel from "./StickerPanel.svelte";
  import HelpPanel from "./HelpPanel.svelte";
  import { setLayersOpen, ui } from "../ui.svelte";

  const sel = $derived(editor.selected);
  const images = $derived(sel.filter((o) => o.type === "image").length);

  // Tracing needs exactly one picture, stickers at least one: otherwise back
  // to the object view (e.g. after clicking elsewhere on the canvas).
  $effect(() => {
    const tab = editor.rightTab;
    if ((tab === "trace" && !(sel.length === 1 && images === 1)) || (tab === "sticker" && images === 0)) editor.rightTab = "object";
  });

  const context = $derived.by((): { icon: string; title: string } => {
    if (editor.rightTab === "trace") return { icon: "wand", title: t("In Schnittlinien umwandeln") };
    if (editor.rightTab === "sticker") return { icon: "sparkle", title: images > 1 ? t("Sticker-Bogen") : t("Sticker") };
    if (editor.rightTab === "help") return { icon: "help", title: t("Hilfe") };
    if (!sel.length) return { icon: editor.printCut.enabled ? "sticker" : "grid", title: editor.printCut.enabled ? t("Seite") : t("Matte") };
    if (sel.length > 1) return { icon: "layers", title: images === sel.length ? t("{n} Bilder", { n: sel.length }) : t("{n} Objekte", { n: sel.length }) };
    const o = sel[0];
    if (o.type === "image") return { icon: "imagePlus", title: t("Bild") };
    if (o.text) return { icon: "type", title: t("Text") };
    if (o.shape) return { icon: "hexagon", title: t("Form") };
    return { icon: "scissors", title: t("Pfad") };
  });

  const inSub = $derived(editor.rightTab === "trace" || editor.rightTab === "sticker" || editor.rightTab === "help");
</script>

<aside>
  <div class="bar" data-tour="tabs">
    {#if inSub}
      <button class="btn icon back" title={t("Zurück")} aria-label={t("Zurück")} onclick={() => (editor.rightTab = "object")}><Icon name="chevronLeft" /></button>
    {:else}
      <span class="ctx-icon"><Icon name={context.icon} size={15} /></span>
    {/if}
    <span class="ctx">{context.title}</span>
    <button class="btn icon help" class:active={editor.rightTab === "help"} title={t("Hilfe")} data-tour="help" onclick={() => (editor.rightTab = editor.rightTab === "help" ? "object" : "help")}>
      <Icon name="help" size={15} />
    </button>
  </div>

  <div class="body">
    {#if editor.rightTab === "trace"}
      <TracePanel />
    {:else if editor.rightTab === "sticker"}
      <StickerPanel />
    {:else if editor.rightTab === "help"}
      <HelpPanel />
    {:else if sel.length}
      <PropertiesPanel />
    {:else}
      <PagePanel />
    {/if}
  </div>

  <div class="layers" class:open={ui.layersOpen}>
    <button class="layers-head" onclick={() => setLayersOpen(!ui.layersOpen)} aria-expanded={ui.layersOpen}>
      <Icon name="layers" size={14} />
      {t("Ebenen")}
      {#if editor.objects.length}<span class="count">{editor.objects.length}</span>{/if}
      <span class="chev"><Icon name="chevronLeft" size={12} /></span>
    </button>
    {#if ui.layersOpen}
      <div class="layers-body"><LayersPanel /></div>
    {/if}
  </div>
</aside>

<style>
  aside {
    width: 320px;
    flex: none;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border-left: 1px solid var(--border);
    min-height: 0;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 44px;
    flex: none;
    padding: 0 8px 0 14px;
    border-bottom: 1px solid var(--border);
  }
  .bar:has(.back) {
    padding-left: 6px;
  }
  .ctx-icon {
    display: inline-flex;
    color: var(--accent);
  }
  .ctx {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }
  .back {
    width: 28px;
    height: 28px;
  }
  .help {
    width: 28px;
    height: 28px;
    color: var(--text-2);
  }
  .body {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
  }
  .layers {
    flex: none;
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--border);
  }
  .layers.open {
    max-height: 40%;
  }
  .layers-head {
    display: flex;
    align-items: center;
    gap: 7px;
    height: 36px;
    flex: none;
    padding: 0 14px;
    border: 0;
    background: var(--panel);
    color: var(--text-2);
    font-weight: 500;
    cursor: pointer;
  }
  .layers-head:hover {
    color: var(--text);
  }
  .chev {
    display: inline-flex;
    margin-left: auto;
    color: var(--text-3);
    transform: rotate(90deg);
    transition: transform 0.15s;
  }
  .layers.open .chev {
    transform: rotate(-90deg);
  }
  .count {
    font-size: 10px;
    padding: 0 5px;
    border-radius: 999px;
    background: var(--panel-2);
    color: var(--text-2);
  }
  .layers-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    border-top: 1px solid var(--border);
  }
</style>
