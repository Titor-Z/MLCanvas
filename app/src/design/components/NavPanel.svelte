<script lang="ts">
  import { designStore } from "../designStore.svelte.js";
  import { artboardService } from "../../lib/services/artboardService.js";
  import ResourcePanel from "./ResourcePanel.svelte";
  import MarketPanel from "./MarketPanel.svelte";
  import { scrollFade } from "../../lib/scrollFade.js";

  /** 右键菜单：目标画板 + 视口坐标；confirm 非空时菜单切换为删除确认 */
  let menu = $state<{ file: string; x: number; y: number } | null>(null);
  let confirmDelete = $state(false);
  /** 双击重命名的目标画板 */
  let renaming = $state<string | null>(null);
  let renameValue = $state("");
  let renameInput = $state<HTMLInputElement | null>(null);

  const boards = $derived(designStore.filtered);

  function openMenu(e: MouseEvent, file: string) {
    e.preventDefault();
    designStore.select(file);
    confirmDelete = false;
    menu = {
      file,
      x: Math.min(e.clientX, window.innerWidth - 150),
      y: Math.min(e.clientY, window.innerHeight - 110),
    };
  }

  function closeMenu() {
    menu = null;
    confirmDelete = false;
  }

  function startRename(file: string) {
    closeMenu();
    renaming = file;
    renameValue = designStore.boards.find((b) => b.id === file)?.name ?? "";
    requestAnimationFrame(() => renameInput?.focus());
  }

  async function commitRename() {
    const file = renaming;
    renaming = null;
    if (!file) return;
    const title = renameValue.trim();
    if (!title || title === (designStore.boards.find((b) => b.id === file)?.name ?? "")) return;
    try {
      await artboardService.setTitle(file, title);
      await designStore.refresh();
    } catch (e) {
      designStore.addLog(String(e).replace(/^.*Error:\s*/, "").trim() || "重命名失败");
    }
  }

  async function doDelete() {
    const file = menu?.file;
    if (!file) return;
    closeMenu();
    try {
      await artboardService.remove(file);
      if (designStore.selectedId === file) designStore.selectedId = null;
      await designStore.refresh();
    } catch (e) {
      designStore.addLog(String(e).replace(/^.*Error:\s*/, "").trim() || "删除失败");
    }
  }

  // ── 拖拽排序（指针实现：HTML5 DnD 在 WKWebView 里投放事件不可靠）──
  let listEl = $state<HTMLElement | null>(null);
  /** 按下待定：越过 5px 阈值才进入拖拽（区分点击） */
  let pending = $state<{ file: string; startY: number } | null>(null);
  /** 拖拽中：ghost 跟手的几何信息 */
  let drag = $state<{ file: string; name: string; left: number; width: number; offsetY: number; y: number } | null>(null);
  /** 插入指示线（第 at 个其他行之前） */
  let insertAt = $state<number | null>(null);
  let lineY = $state(0);
  let lineLeft = $state(0);
  let lineWidth = $state(0);
  let suppressClickUntil = 0;

  function rows(): HTMLElement[] {
    return Array.from(listEl?.querySelectorAll(".item") ?? []) as HTMLElement[];
  }

  function onItemPointerDown(e: PointerEvent, file: string) {
    if (e.button !== 0 || renaming !== null) return;
    if ((e.target as HTMLElement).tagName === "INPUT") return;
    pending = { file, startY: e.clientY };
  }

  function onWindowPointerMove(e: PointerEvent) {
    if (pending && !drag) {
      if (Math.abs(e.clientY - pending.startY) < 5) return;
      const row = rows().find((el) => (el as HTMLElement).dataset.file === pending!.file);
      const r = (row ?? listEl)!.getBoundingClientRect();
      drag = {
        file: pending.file,
        name: designStore.boards.find((b) => b.id === pending!.file)?.name ?? "",
        left: r.left, width: r.width,
        offsetY: e.clientY - r.top, y: e.clientY,
      };
    }
    if (!drag) return;
    drag = { ...drag, y: e.clientY };
    // 插入位：在其他行（不含被拖行）的中点处判定
    const others = rows().filter((el) => el.dataset.file !== drag!.file);
    const rects = others.map((el) => el.getBoundingClientRect());
    let at = others.length;
    for (let i = 0; i < others.length; i++) {
      if (e.clientY < rects[i].top + rects[i].height / 2) { at = i; break; }
    }
    insertAt = at;
    lineLeft = rects.length ? rects[0].left : 0;
    lineWidth = rects.length ? rects[0].width : 0;
    lineY = at < rects.length ? rects[at].top - 1 : rects.length ? rects[rects.length - 1].bottom - 1 : 0;
  }

  function onWindowPointerUp() {
    const d = drag;
    const at = insertAt;
    drag = null; pending = null; insertAt = null;
    if (!d || at === null) return;
    suppressClickUntil = Date.now() + 300;
    const visible = boards.map((b) => b.id).filter((f) => f !== d.file);
    visible.splice(at, 0, d.file);
    const full = designStore.boards.map((b) => b.id);
    if (visible.join("\n") === full.join("\n")) return;
    // 全量顺序合并：可见项按新顺序，搜索未命中的项保持原相对位置
    const queue = [...visible];
    const merged = full.map((f) => (visible.includes(f) ? (queue.shift() as string) : f));
    artboardService
      .saveOrder(merged)
      .then(() => designStore.refresh())
      .catch((e) => designStore.addLog(String(e).replace(/^.*Error:\s*/, "").trim() || "排序失败"));
  }
</script>

{#if designStore.panelOpen}
  <aside class="nav" data-id="nav">
    {#if designStore.panel === "design"}
      <div class="nav__search">
        <input placeholder="搜索画板" bind:value={designStore.searchText} />
      </div>
      <div class="nav__section"><span>画板 · {designStore.filtered.length}</span></div>
      <div class="nav__list" bind:this={listEl} use:scrollFade>
        {#each boards as board (board.id)}
          <div
            class="item"
            class:active={designStore.selectedId === board.id}
            class:dragging={drag?.file === board.id}
            role="button"
            tabindex="0"
            data-file={board.id}
            onclick={() => {
              if (Date.now() < suppressClickUntil) return;
              designStore.select(board.id, true);
            }}
            ondblclick={() => startRename(board.id)}
            oncontextmenu={(e) => openMenu(e, board.id)}
            onpointerdown={(e) => onItemPointerDown(e, board.id)}
          >
            <div class="item__thumb"></div>
            {#if renaming === board.id}
              <input
                class="item__rename"
                bind:this={renameInput}
                bind:value={renameValue}
                onkeydown={(e) => {
                  if (e.key === "Enter") commitRename();
                  if (e.key === "Escape") renaming = null;
                }}
                onblur={commitRename}
                onclick={(e) => e.stopPropagation()}
              />
            {:else}
              <div class="item__name">{board.name}</div>
            {/if}
          </div>
        {:else}
          <div class="nav__empty">
            {designStore.boards.length === 0
              ? "还没有画板。打开 Agent，描述你的第一个界面。"
              : "没有匹配的画板。"}
          </div>
        {/each}
      </div>
    {:else if designStore.panel === "resources"}
      <ResourcePanel />
    {:else}
      <MarketPanel />
    {/if}
  </aside>
{/if}

{#if drag}
  <!-- 跟手 ghost：被拖行的浮起副本（不接收事件） -->
  <div class="drag-ghost" style="left:{drag.left}px;top:{drag.y - drag.offsetY}px;width:{drag.width}px">
    <div class="item__thumb"></div>
    <div class="item__name">{drag.name}</div>
  </div>
  <!-- 插入位置指示线 -->
  <div class="drag-line" style="left:{lineLeft}px;top:{lineY}px;width:{lineWidth}px"></div>
{/if}

{#if menu}
  <div class="ctx" style="left:{menu.x}px;top:{menu.y}px" role="menu">
    {#if confirmDelete}
      <div class="ctx__confirm">
        <p>删除这块画板？</p>
        <div class="ctx__actions">
          <button class="ctx__btn" onclick={closeMenu}>取消</button>
          <button class="ctx__btn ctx__btn--danger" onclick={doDelete}>删除</button>
        </div>
      </div>
    {:else}
      <button class="ctx__item" onclick={() => startRename(menu.file)}>重命名</button>
      <button class="ctx__item ctx__item--danger" onclick={() => (confirmDelete = true)}>删除</button>
    {/if}
  </div>
  <!-- 点击任意处关闭菜单 -->
  <div class="ctx__scrim" onclick={closeMenu}></div>
{/if}

<svelte:window onpointermove={onWindowPointerMove} onpointerup={onWindowPointerUp} />

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
  .nav__list { flex: 1; overflow-y: auto; }
  .item {
    display: flex; align-items: center; gap: 10px;
    padding: 8px 12px;
    border-left: 2px solid transparent;
    cursor: default;
    color: var(--text-2);
    user-select: none;
  }
  .item:hover { background: rgba(255, 255, 255, 0.04); color: var(--text); }
  .item.active { background: var(--accent-soft); border-left-color: var(--accent); color: var(--accent); font-weight: 600; }
  .item.dragging { opacity: 0.35; }
  /* 画板缩略图（方案B）：统一小白卡 = 画板的微缩，投影分层，渐变 hero + 内容条 */
  .item__thumb {
    width: 30px; height: 38px; flex-shrink: 0;
    background: #fff; border-radius: 3px;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
    position: relative; overflow: hidden;
  }
  .item__thumb::before { content: ""; position: absolute; inset: 0 0 55%; background: linear-gradient(135deg, #1473e688, #7a5cff66); }
  .item__thumb::after { content: ""; position: absolute; inset: 52% 20% auto; height: 4px; border-radius: 2px; background: #d8dee6; }
  .item__name { font-size: 12px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .item__rename {
    flex: 1; min-width: 0;
    height: 24px; padding: 0 6px; font-size: 12px;
  }
  .nav__empty {
    padding: 8px 12px;
    font-size: 12px; line-height: 1.6;
    color: var(--text-3);
  }
  /* 拖拽 ghost 与插入指示线（fixed，不接收事件） */
  .drag-ghost {
    position: fixed; z-index: 70;
    display: flex; align-items: center; gap: 10px;
    padding: 8px 12px;
    background: var(--chrome);
    border: 1px solid var(--accent);
    border-radius: var(--radius-btn);
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
    pointer-events: none;
    color: var(--text);
  }
  .drag-line {
    position: fixed; z-index: 70;
    height: 2px; border-radius: 1px;
    background: var(--accent);
    pointer-events: none;
  }
  /* 右键菜单 */
  .ctx {
    position: fixed; z-index: 60;
    min-width: 132px;
    background: var(--chrome);
    border: 1px solid var(--line);
    border-radius: var(--radius-btn);
    box-shadow: 0 8px 28px rgba(0, 0, 0, 0.45);
    padding: 4px;
  }
  .ctx__item {
    display: block; width: 100%;
    padding: 6px 10px;
    border-radius: 4px;
    font-size: 12px; text-align: left;
    color: var(--text-2);
  }
  .ctx__item:hover { background: var(--accent); color: #fff; }
  .ctx__item--danger:hover { background: #e5484d; }
  .ctx__confirm { padding: 8px 10px 10px; }
  .ctx__confirm p { font-size: 12px; color: var(--text); margin-bottom: 10px; }
  .ctx__actions { display: flex; justify-content: flex-end; gap: 6px; }
  .ctx__btn {
    height: 24px; padding: 0 10px;
    border-radius: 4px; font-size: 12px;
    color: var(--text-2);
  }
  .ctx__btn:hover { background: var(--card); color: var(--text); }
  .ctx__btn--danger { background: #e5484d; color: #fff; }
  .ctx__btn--danger:hover { background: #d63b40; color: #fff; }
  .ctx__scrim { position: fixed; inset: 0; z-index: 50; }
</style>
