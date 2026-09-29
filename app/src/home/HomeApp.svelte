<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { projectService, type ProjectInfo } from "../lib/services/projectService.js";
  import ProjectGrid from "./ProjectGrid.svelte";
  import NewProjectDialog from "./NewProjectDialog.svelte";
  import ViewToggle from "./ViewToggle.svelte";

  let projects = $state<ProjectInfo[]>([]);
  let agentConnected = $state(false);
  let dialogOpen = $state(false);
  let view = $state<"grid" | "list">(
    localStorage.getItem("mlcanvas-home-view") === "list" ? "list" : "grid",
  );

  const greeting = $derived.by(() => {
    const h = new Date().getHours();
    if (h < 6) return "夜深了。";
    if (h < 12) return "早上好。";
    if (h < 18) return "下午好。";
    return "晚上好。";
  });

  function setView(v: "grid" | "list") {
    view = v;
    localStorage.setItem("mlcanvas-home-view", v);
  }

  async function refresh() {
    try {
      projects = await projectService.list();
    } catch {
      projects = [];
    }
  }

  async function handleCreate(name: string) {
    await projectService.create(name);
    dialogOpen = false;
    await refresh();
    await projectService.open(name);
  }

  onMount(async () => {
    try {
      agentConnected = await projectService.agentConnected();
    } catch {
      agentConnected = false;
    }
    await refresh();
  });
</script>

<div class="home">
  <header class="titlebar" data-tauri-drag-region>
    <div class="titlebar__logo"><i></i><span>MLCanvas</span></div>
    <div class="titlebar__spacer" data-tauri-drag-region></div>
    <div class="titlebar__btn" title="设置">
      <svg viewBox="0 0 16 16"><path d="M2 4.5h5.5M10.5 4.5H14M2 11.5h5.5M10.5 11.5H14"/><circle cx="9" cy="4.5" r="1.8"/><circle cx="5.5" cy="11.5" r="1.8"/></svg>
    </div>
    <div class="titlebar__agent" data-id="agent-status">
      <i class:off={!agentConnected}></i>{agentConnected ? "Agent 已连接" : "Agent 未连接"}
    </div>
    <div class="winctl">
      <button class="winctl__btn" title="最小化" onclick={() => getCurrentWindow().minimize()}>
        <svg viewBox="0 0 11 11"><path d="M1 5.5h9"/></svg>
      </button>
      <button class="winctl__btn" title="最大化" onclick={() => getCurrentWindow().toggleMaximize()}>
        <svg viewBox="0 0 11 11"><rect x="1.5" y="1.5" width="8" height="8" rx="1"/></svg>
      </button>
      <button class="winctl__btn winctl__btn--close" title="关闭" onclick={() => getCurrentWindow().close()}>
        <svg viewBox="0 0 11 11"><path d="M1.5 1.5l8 8M9.5 1.5l-8 8"/></svg>
      </button>
    </div>
  </header>

  <main class="main">
    <div class="content">
      <h1 class="hero__title">{greeting}</h1>
      <p class="hero__sub">打开你的 Agent，说出你想要的界面——渲染结果会实时出现在画布上。</p>

      <div class="section__head">
        <span class="section__title">项目</span>
        <ViewToggle {view} onset={setView} />
      </div>

      <ProjectGrid {projects} {view} onnew={() => (dialogOpen = true)} onopen={(name) => projectService.open(name)} />
    </div>
  </main>

  {#if dialogOpen}
    <NewProjectDialog oncancel={() => (dialogOpen = false)} oncreate={handleCreate} />
  {/if}
</div>

<style>
  .home {
    display: grid;
    grid-template-rows: 44px 1fr;
    height: 100%;
    background: var(--bg);
  }

  .titlebar {
    display: flex; align-items: center;
    background: var(--chrome);
    border-bottom: 1px solid var(--line);
    padding: 0 8px 0 12px;
  }
  .titlebar__logo { display: flex; align-items: center; gap: 8px; }
  .titlebar__logo i {
    width: 18px; height: 18px; border-radius: 5px;
    background: linear-gradient(135deg, #4b9eff, #7a5cff);
  }
  .titlebar__logo span { font-size: 13px; font-weight: 700; }
  .titlebar__spacer { flex: 1; }
  .titlebar__btn {
    width: 28px; height: 28px;
    display: grid; place-items: center;
    border-radius: var(--radius-btn);
    color: var(--text-2);
  }
  .titlebar__btn:hover { background: rgba(255, 255, 255, 0.08); color: var(--text); }
  .titlebar__btn svg { width: 15px; height: 15px; fill: none; stroke: currentColor; stroke-width: 1.4; stroke-linecap: round; stroke-linejoin: round; }
  .titlebar__agent {
    display: flex; align-items: center; gap: 7px;
    height: 26px; padding: 0 12px;
    border: 1px solid var(--line);
    border-radius: 13px;
    font-size: 11px; letter-spacing: 0.01em;
    color: var(--text-2);
    margin: 0 10px 0 8px;
  }
  .titlebar__agent i { width: 6px; height: 6px; border-radius: 50%; background: var(--ok); }
  .titlebar__agent i.off { background: var(--text-3); }
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

  .main { overflow: hidden; padding: 64px 48px 48px; }
  .content { max-width: 640px; margin: 0 auto; }

  .hero__title { font-size: 24px; font-weight: 700; letter-spacing: -0.02em; line-height: 1.2; }
  .hero__sub { margin-top: 8px; font-size: 13px; line-height: 1.5; letter-spacing: 0.01em; color: var(--text-2); }

  .section__head {
    margin: 40px 0 16px;
    display: flex; align-items: center; justify-content: space-between;
  }
  .section__title { font-size: 14px; font-weight: 700; }
</style>
