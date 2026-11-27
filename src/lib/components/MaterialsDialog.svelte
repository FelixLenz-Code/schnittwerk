<script lang="ts">
  // Own materials in a dialog of their own (menu Plotter → Materialien …).
  import { t } from "../i18n.svelte";
  import { ui } from "../ui.svelte";
  import Icon from "./Icon.svelte";
  import MaterialsEditor from "./MaterialsEditor.svelte";

  const close = () => (ui.materialsOpen = false);
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

<div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && close()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label={t("Eigene Materialien")}>
    <header>
      <h2><Icon name="vinyl" size={18} /> {t("Eigene Materialien")}</h2>
      <button class="btn icon" onclick={close} aria-label={t("Schließen")}><Icon name="x" /></button>
    </header>
    <div class="body">
      <p class="intro">{t("Eigene Materialien erscheinen im Schneiden-Dialog zur Auswahl. Änderungen werden sofort gespeichert.")}</p>
      <MaterialsEditor />
    </div>
    <footer>
      <span class="spacer"></span>
      <button class="btn primary" onclick={close}>{t("Fertig")}</button>
    </footer>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 70;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
    background: rgba(10, 10, 16, 0.45);
    backdrop-filter: blur(2px);
  }
  .dialog {
    display: flex;
    flex-direction: column;
    width: min(480px, 100%);
    max-height: 100%;
    border-radius: 14px;
    background: var(--panel);
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.35);
    overflow: hidden;
  }
  header,
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 16px;
  }
  header {
    justify-content: space-between;
    border-bottom: 1px solid var(--border);
  }
  footer {
    border-top: 1px solid var(--border);
  }
  .spacer {
    flex: 1;
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    font-size: 15px;
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 14px 16px 16px;
    overflow-y: auto;
  }
  .intro {
    margin: 0;
    color: var(--text-2);
    font-size: 12.5px;
    line-height: 1.45;
  }
</style>
