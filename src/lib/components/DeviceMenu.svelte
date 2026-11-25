<script lang="ts">
  import { api } from "../api";
  import { editor } from "../editor.svelte";
  import { plotter } from "../plotter.svelte";
  import type { BleDevice } from "../types";
  import Icon from "./Icon.svelte";
  import { fmt } from "../geometry";
  import { locale, t, tMsg } from "../i18n.svelte";

  let open = $state(false);
  let confirmBlade = $state(false);
  let scanning = $state(false);
  let connecting = $state<string | null>(null);
  let devices = $state<BleDevice[]>([]);
  let scanned = $state(false);

  const STATE: Record<string, string> = { ready: "bereit", moving: "bewegt sich", unloaded: "Matte laden", cutting: "schneidet", unknown: "bereit" };

  async function scan() {
    scanning = true;
    devices = [];
    try {
      devices = await api.bleScan();
    } catch (e) {
      editor.error(e);
    } finally {
      scanning = false;
      scanned = true;
    }
  }

  async function connect(d: BleDevice) {
    connecting = d.id;
    try {
      const name = await api.bleConnect(d.id);
      plotter.rememberBle({ id: d.id, name: d.name || name });
      editor.toast(t("{name} per Bluetooth verbunden – verbindet sich künftig automatisch", { name }), "ok");
      open = false;
      plotter.poll();
    } catch (e) {
      editor.error(e);
    } finally {
      connecting = null;
    }
  }

  async function disconnect() {
    try {
      // A deliberate disconnect also stops the automatic reconnect.
      plotter.forgetBle();
      await api.plotterDisconnect();
      plotter.poll();
    } catch (e) {
      editor.error(e);
    }
  }
</script>

<svelte:window onpointerdown={(e) => open && !(e.target as Element).closest(".device-menu") && (open = false)} />

<div class="device-menu">
  <button
    class="device"
    class:ok={plotter.status.connected && plotter.status.state !== "unloaded"}
    class:warn={plotter.status.state === "unloaded"}
    class:busy={plotter.status.state === "cutting" || plotter.status.state === "moving"}
    class:err={plotter.status.state === "error"}
    title={tMsg(plotter.status.message ?? "Plotter-Verbindung")}
    onclick={() => (open = !open)}
  >
    <span class="dot"></span>
    {#if plotter.status.connected}
      {#if plotter.status.connection === "Bluetooth"}<span class="bt">BT</span>{/if}
      {plotter.status.model?.replace("Silhouette ", "")} · {t(STATE[plotter.status.state] ?? "")}
    {:else if plotter.reconnecting}
      {t("Verbinde per Bluetooth …")}
    {:else if plotter.status.state === "error"}
      {t("Plotter-Fehler")}
    {:else}
      {t("Kein Plotter")}
    {/if}
  </button>

  {#if open}
    <div class="pop">
      <div class="head">
        {#if plotter.status.connected}
          <strong>{plotter.status.model}</strong>
          <span class="muted">{t("verbunden über {c}", { c: plotter.status.connection ?? "" })}</span>
        {:else}
          <strong>{t("Kein Plotter verbunden")}</strong>
          <span class="muted">{tMsg(plotter.status.message ?? "Per USB anschließen – wird automatisch erkannt.")}</span>
        {/if}
      </div>
      {#if plotter.status.connection === "Bluetooth"}
        <button class="btn outline" onclick={disconnect} disabled={plotter.busy}>{t("Bluetooth trennen")}</button>
      {/if}
      {#if plotter.knownBle}
        <div class="known">
          <span class="muted small">
            {t("Automatisch verbinden mit")} <strong>{plotter.knownBle.name || "Plotter"}</strong>
            {#if plotter.reconnecting}– {t("wird gesucht …")}{/if}
          </span>
          {#if plotter.status.connection !== "Bluetooth"}
            <button class="btn small-btn" onclick={() => plotter.forgetBle()}>{t("Vergessen")}</button>
          {/if}
        </div>
      {/if}
      <div class="sep"></div>
      <button class="btn outline" onclick={scan} disabled={scanning || plotter.busy}>
        <Icon name="usb" /> {scanning ? t("Suche per Bluetooth …") : t("Per Bluetooth suchen")}
      </button>
      {#if devices.length}
        <ul>
          {#each devices as d (d.id)}
            <li>
              <div>
                <div class="name">{d.name || t("Unbenannt")}</div>
                <div class="muted small">{d.address}{d.rssi !== null ? ` · ${d.rssi} dBm` : ""}</div>
              </div>
              <button class="btn primary" disabled={!!connecting} onclick={() => connect(d)}>
                {connecting === d.id ? t("Verbinde …") : t("Verbinden")}
              </button>
            </li>
          {/each}
        </ul>
      {:else if scanned && !scanning}
        <p class="muted small">{t("Kein Plotter gefunden. Ist er eingeschaltet und Bluetooth am Gerät aktiv?")}</p>
      {/if}
      <div class="sep"></div>
      <div class="blade" class:worn={plotter.bladeWorn}>
        <div class="blade-head">
          <strong>{t("Messer")}</strong>
          <span class="muted small">{t("seit {date} · {n} Schnitte", { date: new Date(plotter.blade.since).toLocaleDateString(locale()), n: plotter.blade.jobs })}</span>
        </div>
        <div class="bar"><div style:width="{Math.min(100, (plotter.blade.mm / 1000 / plotter.blade.limitM) * 100)}%"></div></div>
        <div class="known">
          <span class="small">{t("{a} m von {b} m geschnitten", { a: fmt(plotter.blade.mm / 1000, 1), b: plotter.blade.limitM })}</span>
          <button class="btn small-btn" onclick={() => (confirmBlade = true)} disabled={plotter.busy}>{t("Neues Messer")}</button>
        </div>
        {#if confirmBlade}
          <div class="known">
            <span class="small">{t("Zähler auf 0 setzen?")}</span>
            <span>
              <button class="btn small-btn primary" onclick={() => { plotter.resetBlade(); confirmBlade = false; }}>{t("Ja")}</button>
              <button class="btn small-btn" onclick={() => (confirmBlade = false)}>{t("Nein")}</button>
            </span>
          </div>
        {/if}
        <label class="known small">
          {t("Erinnern nach")}
          <span><input class="limit" type="number" min="10" max="2000" step="10" value={plotter.blade.limitM} onchange={(e) => { plotter.blade.limitM = Math.max(10, +e.currentTarget.value || 150); plotter.saveBlade(); }} /> m</span>
        </label>
        <label class="known small">
          {t("Ton bei Schnittende")}
          <input type="checkbox" checked={plotter.sound} onchange={(e) => plotter.setSound(e.currentTarget.checked)} />
        </label>
        {#if plotter.bladeWorn}
          <p class="small worn-text">{t("Das Messer hat sein Ziel erreicht – Schnittbild prüfen, bei ausgefransten Kanten wechseln.")}</p>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .device-menu {
    position: relative;
  }
  .device {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 10px;
    border-radius: 999px;
    background: var(--panel-2);
    border: 1px solid var(--border);
    color: var(--text-2);
    font-size: 12px;
    cursor: pointer;
    max-width: 160px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .device:hover {
    border-color: var(--border-strong);
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--text-3);
  }
  .device.ok .dot {
    background: var(--ok);
  }
  .device.warn .dot {
    background: var(--warn);
  }
  .device.busy .dot {
    background: var(--accent);
    animation: blink 0.9s infinite alternate;
  }
  .device.err .dot {
    background: var(--danger);
  }
  @keyframes blink {
    to {
      opacity: 0.3;
    }
  }
  .bt {
    font-size: 10px;
    font-weight: 700;
    color: var(--accent);
  }
  .pop {
    position: absolute;
    right: 0;
    top: 38px;
    z-index: 40;
    width: 300px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px;
    border-radius: var(--radius);
    background: var(--panel);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
  }
  .head {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .muted {
    color: var(--text-3);
  }
  .small {
    font-size: 12px;
    margin: 0;
  }
  .known {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .small-btn {
    height: 24px;
    font-size: 12px;
    border: 1px solid var(--border);
  }
  .sep {
    height: 1px;
    background: var(--border);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 8px;
    border-radius: var(--radius-sm);
    background: var(--panel-2);
  }
  .name {
    font-weight: 600;
  }
  .blade {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .blade-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
  }
  .bar {
    height: 6px;
    border-radius: 6px;
    background: var(--panel-2);
    border: 1px solid var(--border);
    overflow: hidden;
  }
  .bar div {
    height: 100%;
    background: var(--ok);
  }
  .blade.worn .bar div {
    background: var(--warn);
  }
  .worn-text {
    color: var(--warn);
  }
  .limit {
    width: 64px;
    height: 22px;
    padding: 0 4px;
    border: 1px solid var(--border);
    border-radius: 5px;
    background: var(--panel-2);
    text-align: right;
  }
</style>
