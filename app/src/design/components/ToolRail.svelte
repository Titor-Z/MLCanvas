<script lang="ts">
  import { designStore } from "../designStore.svelte.js";

  const tools = [
    { id: "select", title: "选择", icon: "M4 2 L13 8.5 L8.8 9.4 L6.5 13.5 Z" },
    { id: "hand", title: "抓手", icon: "M5 7V3.8a1 1 0 0 1 2 0V7m0-2.5a1 1 0 0 1 2 0V7m0-1.5a1 1 0 0 1 2 0V8m0-.5a1 1 0 0 1 2 0v3a4 4 0 0 1-4 4H8a4 4 0 0 1-4-4V8a1 1 0 0 1 1-1z" },
    { id: "annotate", title: "标注", icon: "M11.1 2.3a1.9 1.9 0 0 1 2.6 2.6L5 13.6 2 14l.4-3z" },
  ] as const;

  const panels = [
    { id: "design", title: "设计面板", icon: "M2 5V3.5A1.5 1.5 0 0 1 3.5 2H5M11 2h1.5A1.5 1.5 0 0 1 14 3.5V5M14 11v1.5a1.5 1.5 0 0 1-1.5 1.5H11M5 14H3.5A1.5 1.5 0 0 1 2 12.5V11", fillCenter: true },
    { id: "components", title: "组件面板", icon: "M8 1.8 L10.5 5 L8 8.2 L5.5 5 Z M4.5 7 L7 10.2 L4.5 13.4 L2 10.2 Z M11.5 7 L14 10.2 L11.5 13.4 L9 10.2 Z", fillCenter: false },
    { id: "spec", title: "规范面板", icon: "M2 5V3.5A1.5 1.5 0 0 1 3.5 2H5M11 2h1.5A1.5 1.5 0 0 1 14 3.5V5M14 11v1.5a1.5 1.5 0 0 1-1.5 1.5H11M5 14H3.5A1.5 1.5 0 0 1 2 12.5V11", fillCenter: false },
  ] as const;
</script>

<nav class="tools">
  <div class="tools__top">
    {#each tools as t (t.id)}
      <button
        class="tools__btn"
        class:active={designStore.tool === t.id}
        title={t.title}
        onclick={() => (designStore.tool = t.id)}
      >
        <svg viewBox="0 0 16 16"><path d={t.icon} /></svg>
      </button>
    {/each}
  </div>
  <div class="tools__spacer"></div>
  <div class="tools__panels" data-id="panel-switch">
    {#each panels as p (p.id)}
      <button
        class="tools__panel"
        class:active={designStore.panelOpen && designStore.panel === p.id}
        title={p.title}
        onclick={() => designStore.togglePanel(p.id)}
      >
        <svg viewBox="0 0 16 16">
          <path d={p.icon} />
          {#if p.fillCenter}<circle cx="8" cy="8" r="1.3" fill="currentColor" stroke="none" />{/if}
        </svg>
      </button>
    {/each}
  </div>
</nav>

<style>
  .tools {
    grid-area: tools;
    background: var(--chrome);
    border-right: 1px solid var(--line);
    display: flex; flex-direction: column; align-items: center;
    padding-top: 10px;
  }
  .tools__top { display: flex; flex-direction: column; gap: 4px; }
  .tools__btn {
    width: 30px; height: 30px;
    display: grid; place-items: center;
    border-radius: var(--radius-btn);
    color: var(--text-2);
  }
  .tools__btn svg { width: 15px; height: 15px; fill: none; stroke: currentColor; stroke-width: 1.5; stroke-linecap: round; stroke-linejoin: round; }
  .tools__btn.active { background: var(--accent); color: #fff; }
  .tools__spacer { flex: 1; }
  .tools__panels {
    width: 100%;
    display: flex; flex-direction: column; align-items: center; gap: 4px;
    padding: 8px 0;
    border-top: 1px solid var(--line);
  }
  .tools__panel {
    width: 30px; height: 30px;
    display: grid; place-items: center;
    border-radius: var(--radius-btn);
    color: var(--text-2);
  }
  .tools__panel svg { width: 15px; height: 15px; fill: none; stroke: currentColor; stroke-width: 1.4; stroke-linecap: round; stroke-linejoin: round; }
  .tools__panel svg :global(circle) { stroke: none; }
  .tools__panel.active { background: var(--accent-soft); color: var(--accent); }
</style>
