<script lang="ts">
  // Coach marks: dims the window except the target and explains it.
  import { t } from "../i18n.svelte";
  import { endTour, tour, TOUR_STEPS } from "../tour.svelte";

  let rect = $state<DOMRect | null>(null);
  const step = $derived(tour.step >= 0 ? TOUR_STEPS[tour.step] : null);

  function measure() {
    if (!step) return (rect = null);
    // Several elements may share a target – spotlight all of them.
    const rs = [...document.querySelectorAll(`[data-tour="${step.target}"]`)].map((e) => e.getBoundingClientRect()).filter((r) => r.width);
    if (!rs.length) return (rect = null);
    const l = Math.min(...rs.map((r) => r.left)), tp = Math.min(...rs.map((r) => r.top));
    rect = new DOMRect(l, tp, Math.max(...rs.map((r) => r.right)) - l, Math.max(...rs.map((r) => r.bottom)) - tp);
  }

  $effect(() => {
    void step;
    measure();
  });

  function next() {
    if (tour.step >= TOUR_STEPS.length - 1) endTour();
    else tour.step++;
  }
  function back() {
    if (tour.step > 0) tour.step--;
  }

  const PAD = 6;
  /** Bubble below the target, or above when there is no room; kept on screen. */
  const bubble = $derived.by(() => {
    const w = 300, h = 170;
    if (!rect) return { x: window.innerWidth / 2 - w / 2, y: window.innerHeight / 2 - h / 2 };
    let x = rect.left + rect.width / 2 - w / 2;
    x = Math.max(12, Math.min(window.innerWidth - w - 12, x));
    let y = rect.bottom + PAD + 12;
    if (y + h > window.innerHeight) y = rect.top - PAD - 12 - h;
    // Tall targets (tool rail, right panel): put the bubble beside them.
    if (rect.height > window.innerHeight / 2) {
      y = Math.max(12, rect.top + 60);
      x = rect.left > window.innerWidth / 2 ? rect.left - w - 16 : rect.right + 16;
    }
    return { x, y: Math.max(12, y) };
  });
</script>

<svelte:window
  onresize={measure}
  onkeydown={(e) => {
    if (!step) return;
    if (e.key === "Escape") endTour();
    else if (e.key === "ArrowRight" || e.key === "Enter") next();
    else if (e.key === "ArrowLeft") back();
    else return;
    e.preventDefault();
    e.stopPropagation();
  }}
/>

{#if step}
  <div class="tour" role="dialog" aria-modal="true" aria-label={t("Einführung")}>
    {#if rect}
      <div
        class="spot"
        style:left="{rect.left - PAD}px"
        style:top="{rect.top - PAD}px"
        style:width="{rect.width + PAD * 2}px"
        style:height="{rect.height + PAD * 2}px"
      ></div>
    {:else}
      <div class="dim"></div>
    {/if}
    <div class="bubble" style:left="{bubble.x}px" style:top="{bubble.y}px">
      <div class="count">{tour.step + 1} / {TOUR_STEPS.length}</div>
      <h3>{t(step.title)}</h3>
      <p>{t(step.text)}</p>
      <div class="actions">
        <button class="btn" onclick={endTour}>{t("Überspringen")}</button>
        <span class="grow"></span>
        {#if tour.step > 0}<button class="btn outline" onclick={back}>{t("Zurück")}</button>{/if}
        <button class="btn primary" onclick={next}>{tour.step === TOUR_STEPS.length - 1 ? t("Fertig") : t("Weiter")}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .tour {
    position: fixed;
    inset: 0;
    z-index: 200;
  }
  .dim {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
  }
  .spot {
    position: absolute;
    border-radius: 10px;
    box-shadow:
      0 0 0 2px var(--accent),
      0 0 0 9999px rgba(0, 0, 0, 0.6);
    transition: all 0.2s ease;
    pointer-events: none;
  }
  .bubble {
    position: absolute;
    width: 300px;
    padding: 14px 16px 12px;
    border-radius: var(--radius);
    background: var(--panel);
    border: 1px solid var(--border);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.3);
    transition:
      left 0.2s ease,
      top 0.2s ease;
  }
  .count {
    font-size: 11px;
    color: var(--text-3);
  }
  h3 {
    margin: 2px 0 6px;
    font-size: 15px;
  }
  p {
    margin: 0 0 12px;
    color: var(--text-2);
    line-height: 1.45;
  }
  .actions {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .grow {
    flex: 1;
  }
</style>
