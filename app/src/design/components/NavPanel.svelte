<script lang="ts">
  import { designStore } from "../designStore.svelte.js";
</script>

{#if designStore.panelOpen}
  <aside class="nav" data-id="nav">
    {#if designStore.panel === "design"}
      <div class="nav__search">
        <input placeholder="搜索画板" bind:value={designStore.searchText} />
      </div>
      <div class="nav__section"><span>画板 · {designStore.filtered.length}</span></div>
      {#each designStore.filtered as board (board.id)}
        <button
          class="nav__item"
          class:active={designStore.selectedId === board.id}
          onclick={() => designStore.select(board.id, true)}
        >
          <div class="nav__thumb"></div>
          <div class="nav__name">{board.name}</div>
        </button>
      {:else}
        <div class="nav__empty">
          {designStore.boards.length === 0
            ? "还没有画板。打开 Agent，描述你的第一个界面。"
            : "没有匹配的画板。"}
        </div>
      {/each}
    {:else if designStore.panel === "components"}
      <div class="nav__section"><span>组件库</span></div>
      <div class="nav__empty">
        组件库还是空的。
        <br />Agent 生成的界面里出现的可复用结构，会沉淀在这里。
      </div>
    {:else}
      <div class="nav__section"><span>设计规范</span></div>
      <div class="nav__empty">
        设计规范还没定义。
        <br />颜色、字体、间距的约定将展示在这里。
      </div>
    {/if}
  </aside>
{/if}

<style>
  .nav {
    grid-area: nav;
    background: var(--chrome);
    border-right: 1px solid var(--line);
    display: flex; flex-direction: column;
    overflow: hidden;
  }
  .nav__search { padding: 10px 12px; }
  .nav__search input { width: 100%; height: 28px; padding: 0 9px; font-size: 12px; }
  .nav__section {
    padding: 10px 12px 6px;
    font-size: 10px; letter-spacing: 1px; text-transform: uppercase;
    color: var(--text-3);
  }
  .nav__item {
    display: flex; align-items: center; gap: 10px;
    padding: 6px 12px;
    border-left: 2px solid transparent;
    text-align: left;
    color: var(--text-2);
  }
  .nav__item:hover { background: rgba(255, 255, 255, 0.04); color: var(--text); }
  .nav__item.active { background: var(--accent-soft); border-left-color: var(--accent); color: var(--accent); font-weight: 600; }
  .nav__thumb {
    width: 24px; height: 32px; flex-shrink: 0;
    background: #fff; border: 1px solid var(--line); border-radius: 3px;
  }
  .nav__name { font-size: 12px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .nav__empty {
    padding: 8px 12px;
    font-size: 12px; line-height: 1.6;
    color: var(--text-3);
  }
</style>
