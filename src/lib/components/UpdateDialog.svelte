<script lang="ts">
  import { fmt } from "../geometry";
  import { updater } from "../update.svelte";
  import Icon from "./Icon.svelte";
  import { t, tMsg } from "../i18n.svelte";

  const info = $derived(updater.info);
  const busy = $derived(updater.phase === "downloading" || updater.phase === "installing");
  const pct = $derived(updater.progress.total ? Math.round((updater.progress.done / updater.progress.total) * 100) : 0);

  /** The "Änderungen" section of the release notes (written by the release
   *  workflow); older releases have none. */
  const notes = $derived.by(() => {
    const t = (info?.notes ?? "").replace(/\r/g, "");
    const m = t.match(/^## Änderungen\s*\n([\s\S]*?)(?=^\*\*Installation|^## |$(?![\s\S]))/m);
    return m ? m[1].trim() : "";
  });

  function close() {
    if (!busy) updater.dialogOpen = false;
  }

  const mb = (b: number) => fmt(b / 1e6, 1);
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

<div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && close()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label="Update">
    <header>
      <h2><Icon name="download" size={18} /> {t("Update auf Schnittwerk {v}", { v: info?.latest ?? "" })}</h2>
      <button class="btn icon" onclick={close} disabled={busy} aria-label={t("Schließen")}><Icon name="x" /></button>
    </header>

    {#if info}
      <div class="body">
        <p class="versions">{t("Installiert:")} <b>{info.current}</b> → {t("neu:")} <b>{info.latest}</b></p>
        {#if notes}
          <div class="notes">{notes}</div>
        {:else}
          <p class="hint">{t("Was neu ist, steht auf der Release-Seite.")}</p>
        {/if}

        {#if updater.phase === "downloading"}
          <div class="progress">
            <div class="bar"><div style:width="{pct}%"></div></div>
            <span>{t("Lade herunter … {a} / {b} MB", { a: mb(updater.progress.done), b: mb(updater.progress.total) })}</span>
          </div>
        {:else if updater.phase === "installing"}
          <div class="progress">
            <div class="bar working"><div style:width="100%"></div></div>
            <span>{info.kind === "deb" ? t("Installiere … (bitte Passwort eingeben)") : t("Installiere …")}</span>
          </div>
        {:else if updater.phase === "installed"}
          <p class="ok"><Icon name="check" size={14} /> {t("Version {v} ist installiert. Nach dem Neustart läuft die neue Version.", { v: info.latest })}</p>
        {:else if updater.phase === "error"}
          <p class="err">{tMsg(updater.error ?? "")}</p>
        {/if}

        {#if info.kind === "manual"}
          <p class="hint">{t("Diese Schnittwerk-Kopie wurde nicht als .deb oder Flatpak installiert und kann sich nicht selbst aktualisieren. Die neue Version gibt es auf der Release-Seite.")}</p>
        {:else if !info.asset}
          <p class="hint">{t("Dieses Release enthält kein passendes Paket ({p}). Bitte über die Release-Seite aktualisieren.", { p: info.kind === "deb" ? ".deb" : "Flatpak" })}</p>
        {:else if info.kind === "deb" && updater.phase !== "installed"}
          <p class="hint">{t("Zum Installieren fragt das System nach deinem Passwort.")}</p>
        {/if}
      </div>

      <footer>
        <button class="btn" onclick={() => updater.openPage()}>{t("Release-Seite")}</button>
        <span class="spacer"></span>
        {#if updater.phase === "installed"}
          <button class="btn primary" onclick={() => updater.restart()}>{t("Jetzt neu starten")}</button>
        {:else if info.asset && info.kind !== "manual"}
          <button class="btn outline" onclick={close} disabled={busy}>{t("Später")}</button>
          <button class="btn primary" onclick={() => updater.install()} disabled={busy}>
            <Icon name="download" /> {updater.phase === "error" ? t("Erneut versuchen") : t("Jetzt aktualisieren")}
          </button>
        {:else}
          <button class="btn outline" onclick={close}>{t("Schließen")}</button>
        {/if}
      </footer>
    {/if}
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
    width: min(560px, 100%);
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
    padding: 16px;
    overflow-y: auto;
  }
  .versions {
    margin: 0;
    color: var(--text-2);
  }
  .notes {
    max-height: 220px;
    overflow-y: auto;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: var(--panel-2);
    color: var(--text-2);
    font-size: 12px;
    line-height: 1.5;
    white-space: pre-wrap;
    user-select: text;
  }
  .hint {
    margin: 0;
    color: var(--text-3);
    font-size: 12px;
  }
  .ok {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    color: var(--ok);
  }
  .err {
    margin: 0;
    color: var(--danger);
  }
  .spacer {
    flex: 1;
  }
  .progress {
    display: flex;
    flex-direction: column;
    gap: 6px;
    color: var(--text-2);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
  .bar {
    height: 8px;
    border-radius: 8px;
    background: var(--panel-2);
    border: 1px solid var(--border);
    overflow: hidden;
  }
  .bar div {
    height: 100%;
    background: linear-gradient(90deg, var(--accent), #ff7eb9);
    transition: width 0.3s;
  }
  .bar.working div {
    background: repeating-linear-gradient(-45deg, var(--accent) 0 8px, #ff7eb9 8px 16px);
    background-size: 22.6px 22.6px;
    animation: stripes 0.8s linear infinite;
  }
  @keyframes stripes {
    to {
      background-position: 22.6px 0;
    }
  }
</style>
