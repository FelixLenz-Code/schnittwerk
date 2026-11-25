// Update check against the GitHub releases and one-click install.

import { listen } from "@tauri-apps/api/event";
import { api, inApp, type UpdateInfo } from "./api";
import { editor } from "./editor.svelte";
import { confirmClose } from "./io.svelte";

const AUTO_KEY = "schnittwerk.updateAuto";
const LAST_KEY = "schnittwerk.updateLast";
/** Automatic checks at most this often. */
const INTERVAL_MS = 6 * 3600 * 1000;

type Phase = "idle" | "checking" | "downloading" | "installing" | "installed" | "error";

function loadAuto(): boolean {
  try {
    return localStorage.getItem(AUTO_KEY) !== "0";
  } catch {
    return true;
  }
}

class UpdateStore {
  info = $state<UpdateInfo | null>(null);
  phase = $state<Phase>("idle");
  progress = $state({ done: 0, total: 0 });
  error = $state<string | null>(null);
  dialogOpen = $state(false);
  auto = $state(loadAuto());
  #listening = false;

  /** A newer version is available. */
  get available() {
    return !!this.info?.newer;
  }

  setAuto(v: boolean) {
    this.auto = v;
    try {
      localStorage.setItem(AUTO_KEY, v ? "1" : "0");
    } catch {}
  }

  /** At start: quietly, at most every few hours, only when allowed. */
  async autoCheck() {
    if (!inApp || !this.auto) return;
    let last = 0;
    try {
      last = Number(localStorage.getItem(LAST_KEY) ?? 0);
    } catch {}
    if (Date.now() - last < INTERVAL_MS) return;
    await this.check(true);
  }

  async check(quiet = false) {
    if (!inApp || this.phase === "checking") return;
    this.phase = "checking";
    this.error = null;
    try {
      this.info = await api.updateCheck();
      try {
        localStorage.setItem(LAST_KEY, String(Date.now()));
      } catch {}
      this.phase = "idle";
      if (!quiet && !this.info.newer) editor.toast(`Schnittwerk ${this.info.current} ist aktuell.`, "ok");
      if (!quiet && this.info.newer) this.dialogOpen = true;
    } catch (e) {
      this.phase = quiet ? "idle" : "error";
      this.error = String(e);
      if (!quiet) editor.error(e);
    }
  }

  async install() {
    const asset = this.info?.asset;
    if (!asset) return;
    if (!this.#listening) {
      this.#listening = true;
      listen<{ phase: "download" | "install"; done: number; total: number }>("update-progress", (e) => {
        this.phase = e.payload.phase === "install" ? "installing" : "downloading";
        this.progress = { done: e.payload.done, total: e.payload.total };
      });
    }
    this.phase = "downloading";
    this.progress = { done: 0, total: asset.size };
    this.error = null;
    try {
      await api.updateInstall(asset);
      this.phase = "installed";
    } catch (e) {
      this.phase = "error";
      this.error = String(e);
    }
  }

  /** Restart into the new version (asks about unsaved changes first). */
  async restart() {
    if (!(await confirmClose())) return;
    editor.dirty = false;
    await api.updateRestart();
  }

  openPage() {
    if (this.info) api.openUrl(this.info.url).catch((e) => editor.error(e));
  }
}

export const updater = new UpdateStore();
