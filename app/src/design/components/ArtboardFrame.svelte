<script lang="ts">
  import { onMount } from "svelte";
  import type { Artboard } from "../../lib/services/artboardService.js";
  import { designStore } from "../designStore.svelte.js";

  let { board, ox, oy, z }: { board: Artboard; ox: number; oy: number; z: number } = $props();

  const src = $derived(`${board.url}?v=${designStore.versions[board.id] ?? 0}`);

  let frameEl = $state<HTMLIFrameElement | null>(null);
  /** 画板声明的外部样式没有加载成功（运行时上报）——黄点可见，白屏不再无声 */
  let styleMissing = $state(false);

  // 画板重新加载后由运行时重新上报，旧状态清零
  $effect(() => {
    void src;
    styleMissing = false;
  });

  onMount(() => {
    function onMessage(e: MessageEvent) {
      const data = e.data as { source?: string; type?: string } | null;
      if (!data || data.source !== "yu-artboard") return;
      if (e.source !== frameEl?.contentWindow) return;
      if (data.type === "style-missing") styleMissing = true;
    }
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  });
</script>

<!-- 外层占位 = 缩放后尺寸；内层画板保持原尺寸做 transform 缩放 -->
<div
  class="slot"
  id="board-{board.id}"
  class:selected={designStore.selectedId === board.id}
  style="left:{(board.x - ox) * z + 96}px;top:{(board.y - oy) * z + 96}px;width:{board.width * z}px;height:{board.height * z}px"
  onclick={() => designStore.select(board.id)}
  onkeydown={(e) => e.key === "Enter" && designStore.select(board.id)}
  tabindex="0"
  role="button"
  aria-label={board.name}
>
  <div class="board" style="width:{board.width}px;height:{board.height}px;transform:scale({z})">
    {#if designStore.showLabels}
      <div class="board__label">
        <b>{board.name}</b>
        {#if styleMissing}
          <i class="board__warn" title="样式未加载：画板引用的外部样式没有加载成功"></i>
        {/if}
      </div>
    {/if}
    <iframe class="board__frame" title={board.name} {src} bind:this={frameEl}></iframe>
  </div>
</div>

<style>
  .slot {
    position: absolute;
    cursor: default;
    outline: none;
  }
  .board {
    position: absolute; top: 0; left: 0;
    transform-origin: top left;
    background: #fff;
    box-shadow: var(--board-shadow);
    border-radius: 2px;
    overflow: visible;
  }
  .slot.selected .board { outline: 1.5px solid var(--accent); }
  .board__label {
    position: absolute; top: -20px; left: 1px;
    display: flex; align-items: center; gap: 5px;
    font-size: 11px; color: var(--text-2);
    white-space: nowrap;
  }
  .board__label b { color: var(--text); font-weight: 600; }
  .slot.selected .board__label b { color: var(--accent); }
  .board__warn {
    width: 7px; height: 7px;
    border-radius: 50%;
    background: #e8b339;
    cursor: help;
  }
  .board :global(.board__frame),
  .board__frame { width: 100%; height: 100%; border: 0; display: block; border-radius: 2px; }
</style>
