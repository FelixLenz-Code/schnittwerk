<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, inApp } from "../api";
  import { customMat, editor, MATS, PAGES, rollMat, ROLL_MAX, ROLL_WIDTH, type PageName, type ProjectKind } from "../editor.svelte";
  import { fmt } from "../geometry";
  import { clearAutosave, confirmDiscard, forgetRecent, leaveDocument, openProject, recent, restoreRecovery, type Recovery, type RecentFile } from "../io.svelte";
  import { loadCustom, PRESETS, rememberMaterial, type Material } from "../materials";
  import { applyTemplate, TEMPLATES } from "../templates.svelte";
  import { setWizardAtStart, wizardUi } from "../wizard.svelte";
  import Icon from "./Icon.svelte";
  import Segmented from "./Segmented.svelte";
  import { updater } from "../update.svelte";
  import { getVersion } from "@tauri-apps/api/app";

  let { recovery = $bindable() }: { recovery: Recovery | null } = $props();

  let version = $state("");
  if (inApp) getVersion().then((v) => (version = v)).catch(() => {});

  type Kind = ProjectKind;

  const KINDS: { id: Kind; icon: string; name: string; hint: string }[] = [
    { id: "sticker", icon: "sticker", name: "Sticker", hint: "Drucken und ausschneiden (Print & Cut)" },
    { id: "vinyl", icon: "vinyl", name: "Vinyl / Klebefolie", hint: "Schriftzüge und Aufkleber aus Plotterfolie" },
    { id: "htv", icon: "shirt", name: "Flexfolie (Textil)", hint: "Zum Aufbügeln – wird gespiegelt geschnitten" },
    { id: "paper", icon: "card", name: "Papier & Karton", hint: "Karten, Anhänger, Lesezeichen, Deko" },
    { id: "pen", icon: "pen", name: "Zeichnen mit Stift", hint: "Linien mit dem Stifthalter zeichnen" },
    { id: "large", icon: "fit", name: "Großes Motiv", hint: "Größer als die Matte – in Kacheln oder von der Rolle" },
  ];

  /** Materials offered per project type; the first one is preselected. */
  const MATERIALS: Record<Kind, string[]> = {
    sticker: ["sticker-matt", "sticker-glossy", "sticker-vinyl", "photo", "paper"],
    vinyl: ["vinyl", "transparency", "magnet"],
    htv: ["htv"],
    paper: ["cardstock", "paper", "photo"],
    pen: ["pen"],
    large: ["vinyl", "htv", "paper", "cardstock"],
  };

  const TIPS: Record<Kind, string> = {
    sticker: "Motiv importieren, im Tab „Sticker“ die Kontur erzeugen, drucken (Strg+P) und den Bogen oben links auf die Matte legen. Die Passermarken werden automatisch gesucht.",
    vinyl: "Folie mit dem Trägerpapier nach unten einlegen. Nach dem Schneiden hilft die Entgitterungshilfe (Tab „Objekt“) beim Abziehen.",
    htv: "Glänzende Trägerfolie nach unten einlegen. Das Motiv wird beim Schneiden automatisch gespiegelt.",
    paper: "Karton braucht meist zwei Durchgänge – vorher einen Testschnitt machen.",
    pen: "Stift statt Messer in den Werkzeughalter setzen. Die Linien werden gezeichnet statt geschnitten.",
    large: "Beim Schneiden wird das Motiv in Kacheln aufgeteilt, die nacheinander geschnitten werden. Eine kleine Überlappung erleichtert das Zusammensetzen.",
  };

  const custom = loadCustom();
  const findMat = (id: string): Material | undefined => [...PRESETS, ...custom].find((m) => m.id === id);

  let step = $state<1 | 2>(1);
  let kind = $state<Kind>("sticker");
  let materialId = $state("sticker-matt");
  let base = $state<"mat" | "roll">("mat");
  let rollLength = $state(600);
  let page = $state<PageName>("A4");
  let stickerStyle = $state<"die" | "kiss">("die");
  let largeW = $state(400);
  let largeH = $state(300);
  let templateId = $state("none");

  const kindInfo = $derived(KINDS.find((k) => k.id === kind)!);
  const materials = $derived(MATERIALS[kind].map(findMat).filter((m): m is Material => !!m));
  /** Own materials (cut dialog) – offered for every type except the pen. */
  const own = $derived(kind === "pen" ? [] : custom.filter((m) => m.tool === "blade"));
  /** Tiles on the mat (same formula as the cut dialog, 5 mm overlap). */
  const matTiles = $derived((largeW <= 203.2 ? 1 : Math.ceil((largeW - 5) / 198.2)) * (largeH <= 304.8 ? 1 : Math.ceil((largeH - 5) / 299.8)));
  const material = $derived(findMat(materialId));
  const templates = $derived(
    kind === "sticker"
      ? TEMPLATES.filter((t) => t.printCut)
      : kind === "paper"
        ? TEMPLATES.filter((t) => ["card", "bookmarks", "tags"].includes(t.id))
        : [],
  );
  const canRoll = $derived(kind === "vinyl" || kind === "htv" || kind === "pen" || kind === "large");
  /** Large design wider than the roll: still tiles, across the roll. */
  const tilesAcross = $derived(kind === "large" && base === "roll" && largeW > ROLL_WIDTH);

  function choose(k: Kind) {
    kind = k;
    materialId = MATERIALS[k][0];
    base = "mat";
    templateId = "none";
    step = 2;
  }

  async function create() {
    if (!(await leaveDocument())) return;
    editor.resetDocument();
    if (kind === "sticker") {
      editor.setPrintCut(true, page);
    } else {
      if (editor.printCut.enabled) editor.setPrintCut(false);
      if (kind === "large") {
        editor.mat = base === "roll" && largeW <= ROLL_WIDTH ? rollMat(largeH) : customMat(largeW, largeH, base === "roll");
      } else {
        editor.mat = base === "roll" ? rollMat(rollLength) : MATS[0];
      }
    }
    editor.material = materialId;
    editor.projectKind = kind;
    rememberMaterial(materialId);
    editor.stickerStyle = stickerStyle;
    const t = templates.find((x) => x.id === templateId);
    if (t) applyTemplate(t);
    editor.dirty = false;
    clearAutosave();
    recovery = null;
    wizardUi.open = false;
    editor.rightTab = kind === "sticker" ? "sticker" : "object";
    requestAnimationFrame(() => editor.fitMat());
  }

  async function blank() {
    if (!(await leaveDocument())) return;
    editor.resetDocument();
    if (editor.printCut.enabled) editor.setPrintCut(false);
    editor.mat = MATS[0];
    clearAutosave();
    recovery = null;
    wizardUi.open = false;
    requestAnimationFrame(() => editor.fitMat());
  }

  async function openRecent(r: RecentFile) {
    if (!(await confirmDiscard())) return;
    if (inApp && !(await api.fileExists(r.path))) {
      forgetRecent(r.path);
      editor.toast(`„${r.name}“ gibt es nicht mehr.`, "error");
      return;
    }
    try {
      await openProject(r.path);
    } catch (e) {
      editor.error(e);
    }
  }

  async function openOther() {
    if (!(await confirmDiscard())) return;
    const path = await open({ filters: [{ name: "Schnittwerk-Projekt", extensions: ["schnitt"] }] });
    if (typeof path !== "string") return;
    try {
      await openProject(path);
    } catch (e) {
      editor.error(e);
    }
  }

  async function restore() {
    if (!recovery || !(await confirmDiscard())) return;
    try {
      await restoreRecovery(recovery);
      recovery = null;
    } catch (e) {
      editor.error(e);
    }
  }

  function discardRecovery() {
    clearAutosave();
    recovery = null;
  }

  function close() {
    wizardUi.open = false;
  }

  const when = (t: number) => {
    const d = new Date(t);
    const today = new Date();
    const time = d.toLocaleTimeString("de-DE", { hour: "2-digit", minute: "2-digit" });
    return d.toDateString() === today.toDateString() ? `heute ${time}` : `${d.toLocaleDateString("de-DE")} ${time}`;
  };
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

<div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && close()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label="Neues Projekt">
    <aside>
      {#if recovery}
        <div class="recovery">
          <div class="rec-title"><Icon name="clock" size={14} /> Nicht gespeicherte Arbeit</div>
          <div class="muted small">„{recovery.name}“ · {recovery.objects} {recovery.objects === 1 ? "Objekt" : "Objekte"} · {when(recovery.savedAt)}</div>
          <div class="rec-actions">
            <button class="btn primary" onclick={restore}>Wiederherstellen</button>
            <button class="btn outline" onclick={discardRecovery}>Verwerfen</button>
          </div>
        </div>
      {/if}
      <h3 class="section-title">Zuletzt geöffnet</h3>
      {#if recent.list.length}
        <ul class="recent">
          {#each recent.list as r (r.path)}
            <li>
              <button class="recent-item" onclick={() => openRecent(r)} title={r.path}>
                {#if r.thumb}<img src={r.thumb} alt="" />{:else}<span class="nothumb"><Icon name="scissors" size={14} /></span>{/if}
                <span class="rtext">
                  <span class="rname">{r.name}</span>
                  <span class="muted small">{when(r.time)}</span>
                </span>
              </button>
              <button class="btn icon forget" title="Aus der Liste entfernen" aria-label="Aus der Liste entfernen" onclick={() => forgetRecent(r.path)}><Icon name="x" size={12} /></button>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="muted small">Noch keine Projekte gespeichert.</p>
      {/if}
      <button class="btn outline open" onclick={openOther}><Icon name="folder" /> Projekt öffnen …</button>
      <div class="bottom">
        <label class="atstart">
          <input type="checkbox" checked={wizardUi.atStart} onchange={(e) => setWizardAtStart(e.currentTarget.checked)} />
          Beim Start anzeigen
        </label>
        <div class="version">
          <span>Schnittwerk {version}</span>
          {#if updater.available}
            <button class="link strong" onclick={() => (updater.dialogOpen = true)}>Update auf {updater.info?.latest}</button>
          {:else}
            <button class="link" onclick={() => updater.check()} disabled={updater.phase === "checking"}>
              {updater.phase === "checking" ? "Prüfe …" : "Nach Updates suchen"}
            </button>
          {/if}
        </div>
        <label class="atstart">
          <input type="checkbox" checked={updater.auto} onchange={(e) => updater.setAuto(e.currentTarget.checked)} />
          Automatisch nach Updates suchen
        </label>
      </div>
    </aside>

    <div class="main">
      <header>
        {#if step === 2}
          <button class="btn icon" onclick={() => (step = 1)} aria-label="Zurück"><Icon name="chevronLeft" /></button>
        {/if}
        <h2>{step === 1 ? "Was möchtest du machen?" : kindInfo.name}</h2>
        <button class="btn icon close" onclick={close} aria-label="Schließen"><Icon name="x" /></button>
      </header>

      {#if step === 1}
        <div class="kinds">
          {#each KINDS as k}
            <button class="kind" onclick={() => choose(k.id)}>
              <span class="kicon"><Icon name={k.icon} size={22} /></span>
              <span class="kname">{k.name}</span>
              <span class="muted small">{k.hint}</span>
            </button>
          {/each}
        </div>
        <div class="foot">
          <button class="btn" onclick={blank}>Leere Matte</button>
        </div>
      {:else}
        <div class="form">
          <section>
            <h3 class="section-title">Material</h3>
            {#if materials.length > 1 || own.length}
              <div class="chips">
                {#each materials as m (m.id)}
                  <button class="chip" class:on={materialId === m.id} onclick={() => (materialId = m.id)}>{m.name}</button>
                {/each}
              </div>
            {/if}
            {#if own.length}
              <div class="chips own">
                <span class="muted small">Eigene:</span>
                {#each own as m (m.id)}
                  <button class="chip" class:on={materialId === m.id} onclick={() => (materialId = m.id)}>{m.name}</button>
                {/each}
              </div>
            {/if}
            {#if material}
              <p class="muted small">
                {material.name}:
                {#if material.tool === "blade"}Messertiefe {material.depth} ·{" "}{/if}Druck {material.pressure} · Geschwindigkeit {material.speed}{material.passes > 1 ? ` · ${material.passes} Durchgänge` : ""}{material.mirror ? " · gespiegelt" : ""}
                – beim Schneiden änderbar.
              </p>
            {/if}
          </section>

          {#if kind === "sticker"}
            <section>
              <h3 class="section-title">Druckseite</h3>
              <Segmented value={page} onchange={(v) => (page = v as PageName)} options={Object.keys(PAGES).map((p) => ({ value: p, label: p }))} />
            </section>
            <section>
              <h3 class="section-title">Sticker-Art</h3>
              <Segmented
                value={stickerStyle}
                onchange={(v) => (stickerStyle = v as "die" | "kiss")}
                options={[
                  { value: "die", label: "Einzeln (mit Trägerrand)" },
                  { value: "kiss", label: "Bogen (nur angeschnitten)" },
                ]}
              />
              <p class="muted small">
                {stickerStyle === "die"
                  ? "Jeder Sticker wird angeschnitten und mit einem Rand durchgeschnitten – fertige Einzelsticker."
                  : "Die Sticker werden nur angeschnitten und bleiben auf dem Bogen."}
              </p>
            </section>
          {:else}
            {#if kind === "large"}
              <section>
                <h3 class="section-title">Größe des Motivs</h3>
                <div class="size-row">
                  <label class="num"><span>B</span><input class="field" type="number" min="50" max="3000" step="10" bind:value={largeW} /><em>mm</em></label>
                  <span class="times">×</span>
                  <label class="num"><span>H</span><input class="field" type="number" min="50" max={ROLL_MAX} step="10" bind:value={largeH} /><em>mm</em></label>
                </div>
              </section>
            {/if}
            <section>
              <h3 class="section-title">Unterlage</h3>
              <Segmented
                value={base}
                onchange={(v) => (base = v as "mat" | "roll")}
                options={canRoll
                  ? [
                      { value: "mat", label: "Schneidematte 8″ × 12″" },
                      { value: "roll", label: "Ohne Matte (Rolle)" },
                    ]
                  : [{ value: "mat", label: "Schneidematte 8″ × 12″" }]}
              />
              {#if base === "roll" && kind !== "large"}
                <div class="size-row">
                  <label class="num wide"><span>Länge</span><input class="field" type="number" min="50" max={ROLL_MAX} step="10" bind:value={rollLength} /><em>mm</em></label>
                </div>
              {/if}
              <p class="muted small">
                {#if base === "roll"}
                  Nur für Folie mit festem Trägerpapier. Breite {fmt(ROLL_WIDTH)} mm{tilesAcross ? ` – das Motiv wird in ${Math.ceil((largeW - 5) / (ROLL_WIDTH - 5))} Bahnen geschnitten` : ""}. Am Ende wird die Folie hinter dem Motiv herausgeschoben.
                {:else if kind === "large"}
                  Schneidebereich 203 × 305 mm – das Motiv wird in {matTiles} {matTiles === 1 ? "Kachel" : "Kacheln"} geschnitten.
                {:else}
                  Material oben links auf die Matte legen.
                {/if}
              </p>
            </section>
          {/if}

          {#if templates.length}
            <section>
              <h3 class="section-title">Vorlage</h3>
              <div class="chips">
                <button class="chip" class:on={templateId === "none"} onclick={() => (templateId = "none")}>Leer</button>
                {#each templates as t (t.id)}
                  <button class="chip" class:on={templateId === t.id} onclick={() => (templateId = t.id)} title={t.hint}>{t.name}</button>
                {/each}
              </div>
            </section>
          {/if}

          <p class="tip"><Icon name="sparkle" size={13} /> {TIPS[kind]}</p>
        </div>
        <div class="foot">
          <button class="btn" onclick={() => (step = 1)}>Zurück</button>
          <button class="btn primary" onclick={create}>Projekt anlegen <Icon name="arrowRight" /></button>
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
    background: rgba(10, 10, 16, 0.45);
    backdrop-filter: blur(2px);
  }
  .dialog {
    display: flex;
    width: min(960px, 100%);
    height: min(620px, 100%);
    border-radius: 14px;
    background: var(--panel);
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.35);
    overflow: hidden;
  }
  aside {
    width: 270px;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 16px;
    background: var(--panel-2);
    border-right: 1px solid var(--border);
    overflow-y: auto;
  }
  .recovery {
    padding: 10px 12px;
    margin-bottom: 8px;
    border: 1px solid var(--warn);
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--warn) 10%, var(--panel));
  }
  .rec-title {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
  }
  .rec-actions {
    display: flex;
    gap: 6px;
    margin-top: 8px;
  }
  .rec-actions .btn {
    flex: 1;
    justify-content: center;
    padding: 0 6px;
  }
  .recent {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .recent li {
    position: relative;
  }
  .recent-item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 6px;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  .recent-item:hover {
    border-color: var(--border);
    background: var(--panel);
  }
  .recent img,
  .nothumb {
    width: 34px;
    height: 46px;
    flex: none;
    object-fit: contain;
    border-radius: 4px;
    border: 1px solid var(--border);
    background: #fff;
  }
  .nothumb {
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-3);
    background: var(--panel);
  }
  .rtext {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .rname {
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .forget {
    position: absolute;
    top: 50%;
    right: 4px;
    transform: translateY(-50%);
    width: 22px;
    height: 22px;
    opacity: 0;
  }
  .recent li:hover .forget {
    opacity: 1;
  }
  .open {
    justify-content: center;
    margin-top: 4px;
  }
  .bottom {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: auto;
    padding-top: 10px;
    border-top: 1px solid var(--border);
  }
  .version {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    color: var(--text-3);
    font-size: 12px;
  }
  .link {
    border: 0;
    background: none;
    padding: 0;
    color: var(--accent);
    font-size: 12px;
    cursor: pointer;
  }
  .link.strong {
    font-weight: 600;
    color: var(--ok);
  }
  .link:disabled {
    color: var(--text-3);
    cursor: default;
  }
  .atstart {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-2);
    font-size: 12px;
    cursor: pointer;
  }
  .atstart input {
    accent-color: var(--accent);
  }
  .main {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  header {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
  }
  h2 {
    margin: 0;
    font-size: 15px;
  }
  .close {
    margin-left: auto;
  }
  .kinds {
    flex: 1;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    align-content: start;
    gap: 12px;
    padding: 16px;
    overflow-y: auto;
  }
  .kind {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    padding: 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--panel-2);
    text-align: left;
    cursor: pointer;
  }
  .kind:hover {
    border-color: var(--accent);
  }
  .kicon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 40px;
    height: 40px;
    margin-bottom: 4px;
    border-radius: 10px;
    background: var(--accent-soft);
    color: var(--accent);
  }
  .kname {
    font-weight: 600;
  }
  .form {
    flex: 1;
    overflow-y: auto;
    padding: 4px 0;
  }
  section {
    padding: 12px 18px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip {
    height: 30px;
    padding: 0 12px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--panel);
    cursor: pointer;
  }
  .chips.own {
    align-items: center;
    margin-top: 8px;
  }
  .chip.on {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
  }
  .size-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 10px;
  }
  .times {
    color: var(--text-3);
  }
  .num {
    position: relative;
    display: flex;
    align-items: center;
    width: 130px;
  }
  .num.wide {
    width: 170px;
  }
  .num span {
    position: absolute;
    left: 8px;
    color: var(--text-3);
    font-size: 11px;
    font-weight: 600;
  }
  .num em {
    position: absolute;
    right: 8px;
    color: var(--text-3);
    font-style: normal;
    font-size: 11px;
  }
  .num .field {
    padding-left: 24px;
    padding-right: 30px;
    text-align: right;
  }
  .num.wide .field {
    padding-left: 52px;
  }
  .muted {
    color: var(--text-3);
    margin: 0;
  }
  .small {
    font-size: 12px;
  }
  section .small {
    margin-top: 8px;
    line-height: 1.45;
  }
  .tip {
    display: flex;
    gap: 6px;
    margin: 6px 18px 12px;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: var(--panel-2);
    color: var(--text-2);
    font-size: 12px;
    line-height: 1.45;
  }
  .tip :global(svg) {
    flex: none;
    margin-top: 2px;
    color: var(--accent);
  }
  .foot {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 16px;
    border-top: 1px solid var(--border);
  }
</style>
