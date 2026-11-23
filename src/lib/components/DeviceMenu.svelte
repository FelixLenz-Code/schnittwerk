<script lang="ts">
  import { api } from "../api";
  import { editor } from "../editor.svelte";
  import { plotter } from "../plotter.svelte";
  import type { BleDevice } from "../types";
  import Icon from "./Icon.svelte";

  let open = $state(false);
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
      editor.toast(`${name} per Bluetooth verbunden`, "ok");
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
    title={plotter.status.message ?? "Plotter-Verbindung"}
    onclick={() => (open = !open)}
  >
    <span class="dot"></span>
    {#if plotter.status.connected}
      {#if plotter.status.connection === "Bluetooth"}<span class="bt">BT</span>{/if}
      {plotter.status.model?.replace("Silhouette ", "")} · {STATE[plotter.status.state] ?? ""}
    {:else if plotter.status.state === "error"}
      Plotter-Fehler
    {:else}
      Kein Plotter
    {/if}
  </button>

  {#if open}
    <div class="pop">
      <div class="head">
        {#if plotter.status.connected}
          <strong>{plotter.status.model}</strong>
          <span class="muted">verbunden über {plotter.status.connection}</span>
        {:else}
          <strong>Kein Plotter verbunden</strong>
          <span class="muted">{plotter.status.message ?? "Per USB anschließen – wird automatisch erkannt."}</span>
        {/if}
      </div>
      {#if plotter.status.connection === "Bluetooth"}
        <button class="btn outline" onclick={disconnect} disabled={plotter.busy}>Bluetooth trennen</button>
      {/if}
      <div class="sep"></div>
      <button class="btn outline" onclick={scan} disabled={scanning || plotter.busy}>
        <Icon name="usb" /> {scanning ? "Suche per Bluetooth …" : "Per Bluetooth suchen"}
      </button>
      {#if devices.length}
        <ul>
          {#each devices as d (d.id)}
            <li>
              <div>
                <div class="name">{d.name || "Unbenannt"}</div>
                <div class="muted small">{d.address}{d.rssi !== null ? ` · ${d.rssi} dBm` : ""}</div>
              </div>
              <button class="btn primary" disabled={!!connecting} onclick={() => connect(d)}>
                {connecting === d.id ? "Verbinde …" : "Verbinden"}
              </button>
            </li>
          {/each}
        </ul>
      {:else if scanned && !scanning}
        <p class="muted small">Kein Plotter gefunden. Ist er eingeschaltet und Bluetooth am Gerät aktiv?</p>
      {/if}
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
</style>
