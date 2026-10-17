<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { designStore } from "../designStore.svelte.js";
  import { Icon } from "../../lib/icons.js";

  interface MenuItem {
    label: string;
    shortcut?: string;
    dim?: boolean;
    action?: () => void;
  }
  interface Menu {
    label: string;
    items: MenuItem[];
  }

  let openMenu = $state<string | null>(null);
  let rootEl = $state<HTMLElement | null>(null);

  let menus: Menu[] = $state([
    {
      label: "文件",
      items: [
        { label: "新建项目", shortcut: "⌘N", action: () => (designStore.dialog = "new") },
        { label: "打开项目…", shortcut: "⌘O", action: () => designStore.openFromDisk() },
        { label: "导入", dim: true },
        { label: "导出", dim: true },
        { label: "项目设置", dim: true },
      ],
    },
    {
      label: "编辑",
      items: [
        { label: "撤销", shortcut: "⌘Z", dim: true },
        { label: "重做", shortcut: "⇧⌘Z", dim: true },
        { label: "设置…", shortcut: "⌘,", action: () => designStore.openSettings("general") },
      ],
    },
    {
      label: "视图",
      items: [
        { label: "放大", shortcut: "⌘+", action: () => designStore.zoomIn() },
        { label: "缩小", shortcut: "⌘−", action: () => designStore.zoomOut() },
        { label: "实际大小", shortcut: "⌘0", action: () => designStore.zoomReset() },
        {
          label: designStore.showLabels ? "隐藏画板标签" : "显示画板标签",
          action: () => (designStore.showLabels = !designStore.showLabels),
        },
      ],
    },
    {
      label: "画板",
      items: [
        { label: "全部刷新", shortcut: "⇧⌘R", action: () => designStore.refresh() },
        { label: "复制", dim: true },
        { label: "重命名", dim: true },
        { label: "删除", dim: true },
      ],
    },
    {
      label: "Agent",
      items: [
        { label: "活动日志", dim: true },
        { label: "管理 Skill…", dim: true },
        { label: "设计规范…", dim: true },
      ],
    },
    {
      label: "帮助",
      items: [{ label: "关于聿Canvas", action: () => designStore.openSettings("about") }],
    },
  ]);

  function toggleMenu(label: string) {
    openMenu = openMenu === label ? null : label;
  }

  function runItem(item: MenuItem) {
    if (item.dim) return;
    openMenu = null;
    item.action?.();
  }

  function onWindowClick(e: MouseEvent) {
    if (rootEl && !rootEl.contains(e.target as Node)) openMenu = null;
  }

  const win = (() => {
    try { return getCurrentWindow(); } catch { return null; }
  })();

  let zoomText = $state("100");
  $effect(() => {
    zoomText = String(designStore.zoom);
  });
  function commitZoom() {
    const n = parseInt(zoomText, 10);
    if (Number.isFinite(n)) designStore.zoom = Math.min(200, Math.max(40, Math.round(n / 10) * 10));
    zoomText = String(designStore.zoom);
  }
</script>

<svelte:window onclick={onWindowClick} />

<header class="titlebar" bind:this={rootEl} data-tauri-drag-region>
  <div class="titlebar__logo" data-id="logo"><span class="logo-yu">聿</span><span class="logo-name">Canvas</span></div>
  <nav class="menus">
    {#each menus as menu (menu.label)}
      <div class="menus__wrap">
        <button
          class="menus__item"
          class:open={openMenu === menu.label}
          onclick={() => toggleMenu(menu.label)}
        >
          {menu.label}
        </button>
        {#if openMenu === menu.label}
          <div class="dropdown">
            {#each menu.items as item (item.label)}
              {#if item.label === "---"}
                <div class="dropdown__sep"></div>
              {:else}
                <button class="dropdown__item" class:dim={item.dim} onclick={() => runItem(item)}>
                  <span>{item.label}</span>
                  {#if item.shortcut}<span class="dropdown__key">{item.shortcut}</span>{/if}
                </button>
              {/if}
            {/each}
          </div>
        {/if}
      </div>
    {/each}
  </nav>
  <div class="titlebar__spacer" data-tauri-drag-region></div>
  <div class="titlebar__doc" data-id="doc-title">{designStore.project}</div>
  <div class="titlebar__spacer" data-tauri-drag-region></div>
  <div class="drawers" data-id="drawers">
    <button
      class="drawer-btn"
      class:on={designStore.drawer === "board"}
      title="画板（⌘⌥1）"
      onclick={() => designStore.toggleDrawer("board")}
    >
      <Icon name="board" size={14} />
    </button>
    <button
      class="drawer-btn"
      class:on={designStore.drawer === "agent"}
      title="对话（⌘J）"
      onclick={() => designStore.toggleDrawer("agent")}
    >
      <Icon name="chat" size={14} />
    </button>
  </div>
  <div class="zoom" data-id="zoom">
    <button class="zoom__btn" title="缩小" onclick={() => designStore.zoomOut()}>−</button>
    <input
      class="zoom__value"
      title="缩放比例"
      bind:value={zoomText}
      onfocus={(e) => e.currentTarget.select()}
      onkeydown={(e) => {
        if (e.key === "Enter") e.currentTarget.blur();
      }}
      onchange={() => commitZoom()}
    />
    <span class="zoom__unit">%</span>
    <button class="zoom__btn" title="放大" onclick={() => designStore.zoomIn()}>＋</button>
  </div>
  <div class="winctl">
    <button class="winctl__btn" title="最小化" onclick={() => win?.minimize()}>
      <svg viewBox="0 0 11 11"><path d="M1 5.5h9"/></svg>
    </button>
    <button class="winctl__btn" title="最大化" onclick={() => win?.toggleMaximize()}>
      <svg viewBox="0 0 11 11"><rect x="1.5" y="1.5" width="8" height="8" rx="1"/></svg>
    </button>
    <button class="winctl__btn winctl__btn--close" title="关闭" onclick={() => win?.close()}>
      <svg viewBox="0 0 11 11"><path d="M1.5 1.5l8 8M9.5 1.5l-8 8"/></svg>
    </button>
  </div>
</header>

<style>
  .titlebar {
    grid-area: titlebar;
    display: flex; align-items: center;
    background: var(--chrome);
    border-bottom: 1px solid var(--line);
    padding: 0 8px 0 12px;
  }
  .titlebar__logo { display: flex; align-items: center; gap: 1px; margin: 0 12px 0 2px; }
  .titlebar__logo .logo-yu {
    font-family: "PingFang SC", sans-serif;
    font-size: 17px; font-weight: 600; line-height: 1;
    background: linear-gradient(135deg, #4b9eff, #7a5cff);
    -webkit-background-clip: text; background-clip: text;
    -webkit-text-fill-color: transparent; color: transparent;
  }
  .titlebar__logo .logo-name { font-size: 13px; font-weight: 700; }
  .menus { display: flex; }
  .menus__wrap { position: relative; }
  .menus__item {
    padding: 4px 9px;
    border-radius: var(--radius-btn);
    font-size: 12px; color: var(--text-2);
  }
  .menus__item:hover { color: var(--text); }
  .menus__item.open { background: var(--accent-soft); color: var(--accent); font-weight: 600; }
  .titlebar__spacer { flex: 1; }
  .titlebar__doc {
    font-size: 12px; font-weight: 600; color: var(--text);
    display: flex; align-items: center; gap: 5px;
  }
  .titlebar__doc::after {
    content: ""; border: 3.5px solid transparent; border-top-color: var(--text-3);
    margin-top: 3px;
  }
  /* 两个抽屉按钮：画板 / 对话，互斥选中 */
  .drawers {
    display: flex; align-items: center; gap: 2px;
    padding: 2px;
    background: rgba(255, 255, 255, 0.045);
    border-radius: 8px;
    margin-right: 4px;
  }
  .drawer-btn {
    width: 28px; height: 24px;
    display: grid; place-items: center;
    border-radius: 6px;
    color: var(--text-3);
    transition: background 140ms ease-out, color 140ms ease-out;
  }
  .drawer-btn:hover { color: var(--text-2); }
  .drawer-btn.on {
    background: var(--raise-2);
    color: var(--text);
    box-shadow: 0 1px 0 rgba(255, 255, 255, 0.07) inset;
  }
  .zoom {
    display: flex; align-items: center; gap: 2px;
    height: 26px; padding: 0 2px;
    border-radius: var(--radius-btn);
    font-size: 12px; color: var(--text-2);
    margin-right: 8px;
  }
  .zoom__btn {
    width: 22px; height: 24px;
    display: grid; place-items: center;
    border-radius: 4px;
    color: var(--text-2);
    font-size: 13px;
  }
  .zoom__btn:hover { background: rgba(255, 255, 255, 0.08); color: var(--text); }
  .zoom__value {
    width: 30px; text-align: right;
    height: 24px; padding: 0 2px;
    font-size: 12px;
    color: var(--text);
    background: none;
    border: 1px solid transparent;
    border-radius: 4px;
  }
  .zoom__value:hover { border-color: var(--line); }
  .zoom__value:focus { border-color: var(--accent); background: var(--field); }
  .zoom__unit { font-size: 12px; color: var(--text-2); margin-right: 2px; }
  .winctl { display: flex; align-items: center; gap: 2px; }
  .winctl__btn {
    width: 28px; height: 28px;
    display: grid; place-items: center;
    border-radius: var(--radius-btn);
    color: var(--text-2);
  }
  .winctl__btn svg { width: 12px; height: 12px; fill: none; stroke: currentColor; stroke-width: 1.2; stroke-linecap: round; stroke-linejoin: round; }
  .winctl__btn:hover { background: rgba(255, 255, 255, 0.08); color: var(--text); }
  .winctl__btn--close:hover { background: #e81123; color: #fff; }

  .dropdown {
    position: absolute; top: 30px; left: 0;
    min-width: 200px;
    background: var(--chrome);
    border: 1px solid var(--line);
    border-radius: var(--radius-btn);
    box-shadow: var(--menu-shadow);
    padding: 5px;
    z-index: 20;
  }
  .dropdown__item {
    display: flex; align-items: center; justify-content: space-between;
    width: 100%;
    height: 28px; padding: 0 10px;
    border-radius: 4px;
    font-size: 12px; color: var(--text);
  }
  .dropdown__item:hover:not(.dim) { background: var(--accent); color: #fff; }
  .dropdown__item:hover:not(.dim) .dropdown__key { color: rgba(255, 255, 255, 0.7); }
  .dropdown__item.dim { color: var(--text-3); cursor: default; }
  .dropdown__key { font-size: 11px; color: var(--text-3); font-family: "SF Mono", Menlo, monospace; }
  .dropdown__sep { height: 1px; margin: 5px 8px; background: var(--line); }
</style>
