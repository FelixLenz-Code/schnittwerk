<script lang="ts">
  // Settings collected in one place (most of them also live elsewhere).
  import { editor } from "../editor.svelte";
  import { fmt } from "../geometry";
  import { i18n, setLang, t } from "../i18n.svelte";
  import { plotter } from "../plotter.svelte";
  import { setTheme, ui, type Theme } from "../ui.svelte";
  import { updater } from "../update.svelte";
  import { setWizardAtStart, wizardUi } from "../wizard.svelte";
  import Icon from "./Icon.svelte";
  import Segmented from "./Segmented.svelte";
  import Toggle from "./Toggle.svelte";

  const close = () => (ui.settingsOpen = false);
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

<div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && close()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label={t("Einstellungen")}>
    <header>
      <h2><Icon name="sliders" size={18} /> {t("Einstellungen")}</h2>
      <button class="btn icon" onclick={close} aria-label={t("Schließen")}><Icon name="x" /></button>
    </header>
    <div class="body">
      <section>
        <h3 class="section-title">{t("Oberfläche")}</h3>
        <div class="row">
          <span>{t("Sprache")}</span>
          <select class="field" value={i18n.lang} onchange={(e) => setLang(e.currentTarget.value as "de" | "en")}>
            <option value="de">Deutsch</option>
            <option value="en">English</option>
          </select>
        </div>
        <div class="row">
          <span>{t("Design")}</span>
          <Segmented
            value={ui.theme}
            onchange={(v) => setTheme(v as Theme)}
            options={[
              { value: "system", label: "Wie das System" },
              { value: "light", label: "Hell" },
              { value: "dark", label: "Dunkel" },
            ]}
          />
        </div>
        <Toggle label="Startseite beim Öffnen zeigen" checked={wizardUi.atStart} onchange={(v: boolean) => setWizardAtStart(v)} />
      </section>
      <section>
        <h3 class="section-title">{t("Arbeitsfläche")}</h3>
        <div class="row">
          <span>{t("Rasterweite beim Verschieben")}</span>
          <select class="field" value={editor.gridStep} onchange={(e) => (editor.gridStep = +e.currentTarget.value)}>
            {#each [0.5, 1, 2, 5, 10] as g}<option value={g}>{fmt(g)} mm</option>{/each}
          </select>
        </div>
        <div class="row">
          <span>{t("Winkelschritt beim Drehen")}</span>
          <select class="field" value={editor.rotStep} onchange={(e) => (editor.rotStep = +e.currentTarget.value)}>
            {#each [1, 5, 15, 45, 90] as a}<option value={a}>{a}°</option>{/each}
          </select>
        </div>
      </section>
      <section>
        <h3 class="section-title">{t("Plotter")}</h3>
        <Toggle label="Ton bei Schnittende" checked={plotter.sound} onchange={(v: boolean) => plotter.setSound(v)} />
        <div class="row">
          <span>{t("Messer wechseln nach")}</span>
          <span>
            <input
              class="field num"
              type="number"
              min="10"
              max="2000"
              step="10"
              value={plotter.blade.limitM}
              onchange={(e) => {
                plotter.blade.limitM = Math.max(10, +e.currentTarget.value || 150);
                plotter.saveBlade();
              }}
            /> m
          </span>
        </div>
      </section>
      <section>
        <h3 class="section-title">{t("Updates")}</h3>
        <Toggle label="Beim Start nach Updates suchen" checked={updater.auto} onchange={(v: boolean) => updater.setAuto(v)} />
      </section>
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
    overflow-y: auto;
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 16px;
    border-bottom: 1px solid var(--border);
  }
  section:last-child {
    border-bottom: 0;
  }
  .section-title {
    margin: 0;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    color: var(--text-2);
  }
  .row select {
    width: 170px;
    flex: none;
  }
  .num {
    width: 72px;
    text-align: right;
  }
</style>
