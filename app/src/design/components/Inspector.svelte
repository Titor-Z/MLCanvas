<script lang="ts">
  import { designStore } from "../designStore.svelte.js";
  import AgentChat from "./AgentChat.svelte";
  import { Icon } from "../../lib/icons.js";

  const selected = $derived(designStore.selected);
  const drawer = $derived(designStore.drawer);
</script>

<aside class="pane" class:pane--board={drawer === "board"} data-id="inspector">
  {#if drawer === "agent"}
    <AgentChat />
  {:else}
    <div class="pane__head">
      <b>画板</b>
      <button class="pane__close" title="收起" onclick={() => (designStore.drawer = null)}>
        <Icon name="close" />
      </button>
    </div>
    <div class="pane__body">
      {#if selected}
        <div class="group">
          <div class="group__title">{selected.name}</div>
          <div class="kv">
            <span class="kv__k">尺寸</span>
            <span class="kv__v kv__v--num">{selected.width}</span>
            <span class="kv__x">×</span>
            <span class="kv__v kv__v--num">{selected.height}</span>
          </div>
          <div class="kv">
            <span class="kv__k">位置</span>
            <span class="kv__v kv__v--num">{Math.round(selected.x)}</span>
            <span class="kv__x">,</span>
            <span class="kv__v kv__v--num">{Math.round(selected.y)}</span>
          </div>
        </div>
        <div class="group">
          <div class="group__title">显示</div>
          <div class="kv">
            <span class="kv__k">标签</span>
            <span class="kv__v">{designStore.showLabels ? "显示" : "隐藏"}</span>
          </div>
        </div>
      {:else}
        <div class="pane__empty">在画布中选择一个画板</div>
      {/if}
    </div>
  {/if}
</aside>

<style>
  .pane {
    grid-area: inspector;
    background: var(--panel);
    border-left: 1px solid var(--line);
    display: flex; flex-direction: column;
    overflow: hidden;
    min-width: 0;
  }
  .pane__head {
    display: flex; align-items: center; gap: 8px;
    padding: 13px 16px 11px;
    flex-shrink: 0;
  }
  .pane__head b { font-size: 13px; font-weight: 600; }
  .pane__close {
    margin-left: auto;
    width: 24px; height: 24px;
    display: grid; place-items: center;
    border-radius: 6px;
    color: var(--text-3);
    transition: background 140ms ease-out, color 140ms ease-out;
  }
  .pane__close:hover { background: rgba(255, 255, 255, 0.07); color: var(--text-2); }
  .pane__body { padding: 2px 16px 16px; display: grid; gap: 14px; align-content: start; }
  .group { display: grid; gap: 7px; }
  .group__title { font-size: 11px; color: var(--text-2); }
  .kv { display: flex; align-items: center; gap: 8px; }
  .kv__k { width: 34px; font-size: 11.5px; color: var(--text-3); flex-shrink: 0; }
  .kv__v {
    flex: 1; height: 28px;
    display: flex; align-items: center;
    padding: 0 10px;
    background: var(--field);
    border: 1px solid var(--line);
    border-radius: 7px;
    font-size: 11.5px;
  }
  .kv__v--num { justify-content: flex-end; font-variant-numeric: tabular-nums; font-family: "SF Mono", Menlo, monospace; font-size: 11px; }
  .kv__x { font-size: 10.5px; color: var(--text-3); }
  .pane__empty { font-size: 12px; color: var(--text-3); }
</style>
