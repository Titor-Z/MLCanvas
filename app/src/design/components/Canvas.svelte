<script lang="ts">
  import { designStore } from "../designStore.svelte.js";
  import ArtboardFrame from "./ArtboardFrame.svelte";

  let canvasEl = $state<HTMLElement | null>(null);

  // 打开项目后自动 fit-all：按画板包围盒一次看全（自由画布）
  $effect(() => {
    if (designStore.loading || designStore.boards.length === 0 || designStore.fitted) return;
    if (!canvasEl) return;
    const bs = designStore.boards;
    const minX = Math.min(...bs.map((b) => b.x));
    const minY = Math.min(...bs.map((b) => b.y));
    const maxX = Math.max(...bs.map((b) => b.x + b.width));
    const maxY = Math.max(...bs.map((b) => b.y + b.height));
    // 可用空间 = 画布视口减去四周 padding（padding 已含在 clientWidth 里，两侧都要减）
    const pad = 96 * 2;
    const availW = canvasEl.clientWidth - pad;
    const availH = canvasEl.clientHeight - pad;
    const z = Math.min(availW / (maxX - minX), availH / (maxY - minY), 1);
    designStore.zoom = Math.min(200, Math.max(20, Math.floor(z * 100)));
    designStore.fitted = true;
    // fit 后内容居中：取整产生的富余均匀分到两侧，而不是全堆在右边
    requestAnimationFrame(() => {
      const el = canvasEl!;
      el.scrollTo({
        left: (el.scrollWidth - el.clientWidth) / 2,
        top: (el.scrollHeight - el.clientHeight) / 2,
      });
    });
  });

  // 画板列表点选：把目标画板滚动到视口中央
  $effect(() => {
    const id = designStore.centerId;
    if (!id || designStore.selectedId !== id) return;
    requestAnimationFrame(() => {
      document.getElementById(`board-${id}`)?.scrollIntoView({
        behavior: "smooth", block: "center", inline: "center",
      });
    });
  });

  // Cmd(macOS)/Ctrl/Alt + 滚轮缩放画布（细步进，手感更平滑）
  function onWheel(e: WheelEvent) {
    if (!(e.metaKey || e.ctrlKey || e.altKey)) return;
    e.preventDefault();
    designStore.zoomStep(e.deltaY < 0 ? 5 : -5);
  }

  // 抓手：工具选择抓手，或按住空格临时切换（Figma/Sketch 惯例）
  let spaceHeld = $state(false);
  let panning = $state(false);
  let dragDist = 0;
  let panStart = { x: 0, y: 0, sl: 0, st: 0 };
  const handActive = $derived(designStore.tool === "hand" || spaceHeld);

  function onKeydown(e: KeyboardEvent) {
    if (e.code !== "Space") return;
    const tag = (e.target as HTMLElement).tagName;
    if (tag === "INPUT" || tag === "TEXTAREA") return;
    e.preventDefault();
    spaceHeld = true;
  }
  function onKeyup(e: KeyboardEvent) {
    if (e.code === "Space") spaceHeld = false;
  }

  function startPan(e: MouseEvent) {
    dragDist = 0;
    if (!handActive || e.button !== 0) return;
    const el = e.currentTarget as HTMLElement;
    panning = true;
    panStart = { x: e.clientX, y: e.clientY, sl: el.scrollLeft, st: el.scrollTop };
    e.preventDefault();
  }
  function onPanMove(e: MouseEvent) {
    if (!panning) return;
    const el = e.currentTarget as HTMLElement;
    const dx = e.clientX - panStart.x;
    const dy = e.clientY - panStart.y;
    dragDist = Math.max(dragDist, Math.hypot(dx, dy));
    el.scrollLeft = panStart.sl - dx;
    el.scrollTop = panStart.st - dy;
  }
  function endPan() {
    panning = false;
  }
  // 拖拽平移后抑制随后的 click（避免误取消选中）
  function onClickSelect() {
    if (dragDist > 3) return;
    designStore.select(designStore.selectedId ?? "");
  }
</script>

<svelte:window onkeydown={onKeydown} onkeyup={onKeyup} />

<main
  class="canvas"
  class:hand={handActive}
  class:panning
  bind:this={canvasEl}
  data-id="canvas"
  onclick={onClickSelect}
  onwheel={onWheel}
  onmousedown={startPan}
  onmousemove={onPanMove}
  onmouseup={endPan}
  onmouseleave={endPan}
>
  {#if designStore.loading}
    <div class="canvas__hint">正在读取画板…</div>
  {:else if designStore.boards.length === 0}
    <div class="canvas__hint">
      <p class="canvas__hint-title">项目画布还是空的。</p>
      <p>打开你的 Agent，描述你想要的第一个界面。</p>
    </div>
  {:else}
    {@const bs = designStore.boards}
    {@const ox = Math.min(0, ...bs.map((b) => b.x))}
    {@const oy = Math.min(0, ...bs.map((b) => b.y))}
    {@const z = designStore.zoom / 100}
    {@const maxX = Math.max(...bs.map((b) => b.x + b.width))}
    {@const maxY = Math.max(...bs.map((b) => b.y + b.height))}
    <div
      class="canvas__flow"
      style="width:{(maxX - ox) * z + 192}px;height:{(maxY - oy) * z + 192}px"
    >
      {#each designStore.boards as board (board.id)}
        <ArtboardFrame {board} {ox} {oy} {z} />
      {/each}
    </div>
  {/if}
</main>

<style>
  .canvas {
    grid-area: canvas;
    position: relative;
    overflow: auto;
    /* WebKit 不把滚动容器的右/下 padding 计入滚动区域，
       留白改由 .canvas__flow 的尺寸兜底（各 +192），这里不设 padding */
  }
  .canvas__flow {
    position: relative;
  }
  .canvas__hint {
    height: 100%;
    display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px;
    color: var(--text-3);
    text-align: center;
  }
  .canvas__hint-title { font-size: 15px; font-weight: 600; color: var(--text-2); }
  /* 抓手模式：光标反馈 + 让画板 iframe 放行鼠标（拖拽才能覆盖画板上方） */
  .canvas.hand { cursor: grab; }
  .canvas.hand :global(iframe) { pointer-events: none; }
  .canvas.panning { cursor: grabbing; }
  .canvas.panning :global(iframe) { pointer-events: none; }
</style>
