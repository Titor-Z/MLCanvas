<script lang="ts">
  import type { Artboard } from "../../lib/services/artboardService.js";
  import { designStore } from "../designStore.svelte.js";

  let { board, ox, oy, z }: { board: Artboard; ox: number; oy: number; z: number } = $props();

  const src = $derived(`${board.url}?v=${designStore.versions[board.id] ?? 0}`);
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
      <div class="board__label"><b>{board.name}</b></div>
    {/if}
    <iframe class="board__frame" title={board.name} {src}></iframe>
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
    font-size: 11px; color: var(--text-2);
    white-space: nowrap;
  }
  .board__label b { color: var(--text); font-weight: 600; }
  .slot.selected .board__label b { color: var(--accent); }
  .board :global(.board__frame),
  .board__frame { width: 100%; height: 100%; border: 0; display: block; border-radius: 2px; }
</style>
