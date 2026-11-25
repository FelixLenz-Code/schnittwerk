<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../api";
  import { t } from "../i18n.svelte";
  import { ui } from "../ui.svelte";
  import Icon from "./Icon.svelte";

  let version = $state("");
  onMount(async () => {
    try {
      const { getVersion } = await import("@tauri-apps/api/app");
      version = await getVersion();
    } catch {
      version = "";
    }
  });
  const close = () => (ui.aboutOpen = false);
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

<div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && close()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label={t("Über Schnittwerk")}>
    <span class="logo"><Icon name="scissors" size={26} stroke={2.2} /></span>
    <h2>Schnittwerk</h2>
    {#if version}<p class="ver">{t("Version {v}", { v: version })}</p>{/if}
    <p>{t("Linux-Software für Silhouette-Schneideplotter")}</p>
    <p class="muted">{t("Freie Software unter der GPL-3.0. Nicht mit Silhouette America verbunden.")}</p>
    <div class="actions">
      <button class="btn outline" onclick={() => api.openUrl("https://github.com/FelixLenz-Code/schnittwerk")}>GitHub</button>
      <button class="btn primary" onclick={close}>{t("Schließen")}</button>
    </div>
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
    background: rgba(10, 10, 16, 0.45);
    backdrop-filter: blur(2px);
  }
  .dialog {
    width: 340px;
    padding: 24px;
    border-radius: 14px;
    background: var(--panel);
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.35);
    text-align: center;
  }
  .logo {
    display: inline-flex;
    padding: 12px;
    border-radius: 14px;
    background: var(--accent);
    color: #fff;
  }
  h2 {
    margin: 10px 0 2px;
    font-size: 18px;
  }
  .ver {
    margin: 0 0 10px;
    color: var(--text-3);
  }
  p {
    margin: 6px 0;
    color: var(--text-2);
  }
  .actions {
    display: flex;
    gap: 8px;
    justify-content: center;
    margin-top: 16px;
  }
</style>
