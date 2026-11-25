<script lang="ts">
  // Collapsible panel section; remembers whether it was open.
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  let { id, title, open: defaultOpen = true, children }: { id: string; title: string; open?: boolean; children: Snippet } = $props();

  const key = () => `schnittwerk.section.${id}`;
  function load(): boolean {
    try {
      const v = localStorage.getItem(key());
      return v === null ? defaultOpen : v === "1";
    } catch {
      return defaultOpen;
    }
  }
  let open = $state(load());
  function toggle() {
    open = !open;
    try {
      localStorage.setItem(key(), open ? "1" : "0");
    } catch {}
  }
</script>

<section class:closed={!open}>
  <button class="head" onclick={toggle} aria-expanded={open}>
    <span class="chev" class:open><Icon name="chevronLeft" size={12} /></span>
    <span class="section-title">{title}</span>
  </button>
  {#if open}
    <div class="content">{@render children()}</div>
  {/if}
</section>

<style>
  section {
    padding: 10px 16px 14px;
    border-bottom: 1px solid var(--border);
  }
  section.closed {
    padding-bottom: 10px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 2px 0;
    border: 0;
    background: none;
    cursor: pointer;
    text-align: left;
  }
  .head .section-title {
    margin: 0;
  }
  .head:hover .section-title {
    color: var(--text-2);
  }
  .chev {
    display: inline-flex;
    color: var(--text-3);
    transform: rotate(-90deg);
    transition: transform 0.12s;
  }
  .chev:not(.open) {
    transform: rotate(180deg);
  }
  .content {
    margin-top: 10px;
  }
</style>
