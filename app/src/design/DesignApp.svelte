<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { designStore } from "./designStore.svelte.js";
  import type { BoardChange } from "./designStore.svelte.js";
  import { agentPresence } from "../lib/agentPresence.svelte.js";
  import { projectService } from "../lib/services/projectService.js";
  import NewProjectDialog from "../home/NewProjectDialog.svelte";
  import SettingsDialog from "../home/SettingsDialog.svelte";
  import TitleBar from "./components/TitleBar.svelte";
  import ToolRail from "./components/ToolRail.svelte";
  import NavPanel from "./components/NavPanel.svelte";
  import Canvas from "./components/Canvas.svelte";
  import Inspector from "./components/Inspector.svelte";

  // 弹窗：新建项目（在当前窗口完成）
  let dlgError = $state("");
  async function handleCreate(name: string, dir?: string) {
    dlgError = "";
    try {
      await projectService.create(name, dir);
      designStore.dialog = null;
      await projectService.open(name);
    } catch (e) {
      dlgError = String(e).replace(/^.*Error:\s*/, "").trim() || "创建项目失败";
    }
  }

  // 画布快捷键（对齐 XD）：⌘/Ctrl + -、+ 缩放，0 回 100%，1 适应全部；
  // ⌘N 新建项目（弹窗）、⌘O 打开项目（系统目录选择器，任意文件夹）。Windows/Linux 用 Ctrl
  function onShortcut(e: KeyboardEvent) {
    if (!(e.metaKey || e.ctrlKey) || e.shiftKey) return;
    const tag = (e.target as HTMLElement).tagName;
    // 抽屉切换是全局的：输入框里按也生效
    if (e.altKey && e.key === "1") {
      e.preventDefault();
      designStore.toggleDrawer("board");
      return;
    }
    if (!e.altKey && (e.key === "j" || e.key === "J")) {
      e.preventDefault();
      designStore.toggleDrawer("agent");
      return;
    }
    if (e.altKey) return;
    if (tag === "INPUT" || tag === "TEXTAREA") return;
    if (e.key === "=" || e.key === "+") {
      e.preventDefault();
      designStore.zoomIn();
    } else if (e.key === "-") {
      e.preventDefault();
      designStore.zoomOut();
    } else if (e.key === "0") {
      e.preventDefault();
      designStore.zoomReset();
    } else if (e.key === "1") {
      e.preventDefault();
      designStore.fitAll(true);
    } else if (e.key === "n" || e.key === "N") {
      e.preventDefault();
      designStore.dialog = "new";
    } else if (e.key === "o" || e.key === "O") {
      e.preventDefault();
      designStore.openFromDisk();
    } else if (e.key === ",") {
      e.preventDefault();
      designStore.openSettings("general");
    }
  }

  onMount(async () => {
    window.addEventListener("keydown", onShortcut);
    agentPresence.init();
    await designStore.init();
    const stopChanged = await listen<{ changes: BoardChange[] }>("artboards-changed", (e) => {
      designStore.onChanged(e.payload?.changes ?? []);
    });
    const stopProject = await listen<string>("project-opened", (e) => {
      designStore.setProject(e.payload);
      designStore.fitted = false;
      designStore.refresh();
    });
    const stopActivity = await listen<{ action: string; file: string | null }>("agent-activity", (e) => {
      designStore.addAgentActivity(e.payload?.action ?? "", e.payload?.file ?? null);
    });
    const stopAgent = await listen<{ state: "idle" | "busy" }>("agent-state", (e) => {
      agentPresence.setBusy(e.payload?.state === "busy");
    });
    const stopPresence = await listen<{ client: string }>("agent-presence", (e) => {
      agentPresence.setClient(e.payload?.client ?? "");
    });
    // 内置助手：流式增量与工具步骤
    const stopDelta = await listen<{ text: string }>("agent-chat-delta", (e) => {
      designStore.onChatDelta(e.payload?.text ?? "");
    });
    const stopTool = await listen<{ tool: string; file: string | null }>("agent-chat-tool", (e) => {
      designStore.onChatTool(e.payload?.tool ?? "", e.payload?.file ?? null);
    });
    // 上下文用量与当前模型（后端在回合内送达）
    const stopUsage = await listen<{ input: number; output: number; used: number; limit: number | null }>(
      "agent-chat-usage",
      (e) => designStore.onChatUsage(e.payload),
    );
    const stopModel = await listen<{ name: string }>("agent-chat-model", (e) => {
      designStore.chatModel = e.payload?.name ?? "";
    });
    return () => {
      window.removeEventListener("keydown", onShortcut);
      stopChanged();
      stopProject();
      stopActivity();
      stopAgent();
      stopPresence();
      stopDelta();
      stopTool();
      stopUsage();
      stopModel();
    };
  });
</script>

<div
  class="design"
  class:panel-closed={!designStore.panelOpen}
  class:drawer-board={designStore.drawer === "board"}
  class:drawer-agent={designStore.drawer === "agent"}
>
  <TitleBar />
  <ToolRail />
  <NavPanel />
  <Canvas />
  {#if designStore.drawer}
    <Inspector />
  {/if}
</div>

{#if designStore.dialog === "new"}
  <NewProjectDialog oncancel={() => (designStore.dialog = null)} oncreate={handleCreate} error={dlgError} />
{:else if designStore.dialog === "settings"}
  <SettingsDialog oncancel={() => (designStore.dialog = null)} onsaved={() => designStore.refreshConfigured()} initialTab={designStore.settingsTab} />
{/if}

<style>
  .design {
    display: grid;
    grid-template-rows: 44px 1fr;
    grid-template-columns: 44px 256px 1fr 0px;
    grid-template-areas:
      "titlebar titlebar titlebar titlebar"
      "tools nav canvas inspector";
    height: 100%;
    background: var(--canvas);
    transition: grid-template-columns 200ms cubic-bezier(0.32, 0.72, 0, 1);
  }
  /* 两个抽屉互斥：对话 440、画板 280 */
  .design.drawer-agent { grid-template-columns: 44px 256px 1fr 440px; }
  .design.drawer-board { grid-template-columns: 44px 256px 1fr 280px; }
  .design.panel-closed { grid-template-columns: 44px 0px 1fr 0px; }
  .design.panel-closed.drawer-agent { grid-template-columns: 44px 0px 1fr 440px; }
  .design.panel-closed.drawer-board { grid-template-columns: 44px 0px 1fr 280px; }
</style>
