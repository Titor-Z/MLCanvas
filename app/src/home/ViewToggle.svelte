<script lang="ts" module>
  interface Props {
    view: "grid" | "list";
    onset: (v: "grid" | "list") => void;
  }
</script>

<script lang="ts">
  let { view, onset }: Props = $props();
</script>

<div class="viewtoggle" role="group">
  <button
    class="viewtoggle__btn"
    class:active={view === "list"}
    title="列表视图"
    onclick={() => onset("list")}
  >
    <svg viewBox="0 0 16 16"><path d="M2.5 4h11M2.5 8h11M2.5 12h11"/></svg>
  </button>
  <button
    class="viewtoggle__btn"
    class:active={view === "grid"}
    title="方块视图"
    onclick={() => onset("grid")}
  >
    <svg viewBox="0 0 16 16"><rect x="2.5" y="2.5" width="4.6" height="4.6" rx="1"/><rect x="8.9" y="2.5" width="4.6" height="4.6" rx="1"/><rect x="2.5" y="8.9" width="4.6" height="4.6" rx="1"/><rect x="8.9" y="8.9" width="4.6" height="4.6" rx="1"/></svg>
  </button>
</div>

<style>
  .viewtoggle {
    display: flex; gap: 2px;
    background: var(--card);
    border: 1px solid var(--line);
    border-radius: var(--radius-btn);
    padding: 2px;
  }
  .viewtoggle__btn {
    width: 26px; height: 24px;
    display: grid; place-items: center;
    border-radius: 4px;
    color: var(--text-3);
  }
  .viewtoggle__btn svg { width: 14px; height: 14px; fill: none; stroke: currentColor; stroke-width: 1.4; stroke-linecap: round; }
  .viewtoggle__btn.active { background: var(--accent-soft); color: var(--accent); }
</style>
