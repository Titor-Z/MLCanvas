import { artboardService, type Artboard } from "../lib/services/artboardService.js";
import { projectService } from "../lib/services/projectService.js";
import { agentService } from "../lib/services/agentService.js";

export interface BoardChange {
  project: string;
  file: string;
  /** MCP 来源（write/register/delete/notify）——活动流据此显示「Agent 更新了」，否则是匿名文件变化 */
  agent?: boolean;
}

export interface LogEntry {
  time: string;
  text: string;
}

export interface ChatEntry {
  id: number;
  kind: "user" | "assistant" | "tool" | "step-end" | "compact";
  text: string;
  /** compact 行：被压缩的条目数 */
  compactCount?: number;
  /** tool 行的执行状态（run → done） */
  toolState?: "run" | "done";
  /** 助手行正在流式输出 */
  streaming?: boolean;
  error?: boolean;
}

function now(): string {
  return new Date().toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", hour12: false });
}

/** 设计窗口状态：画板、选中、面板、缩放、Agent 活动流 */
class DesignStore {
  project = $state("");
  boards = $state<Artboard[]>([]);
  versions = $state<Record<string, number>>({});
  selectedId = $state<string | null>(null);
  tool = $state<"select" | "hand" | "annotate">("select");
  panel = $state<"design" | "resources" | "market">("design");
  panelOpen = $state(true);
  zoom = $state(100);
  /** 打开项目后首次 fit-all 已执行，切换项目时复位 */
  fitted = $state(false);
  showLabels = $state(true);
  logs = $state<LogEntry[]>([]);
  loading = $state(true);
  /** 画板列表点击时置为目标画板，画布收到后将其滚动到视口中央 */
  centerId = $state("");
  /** 画布滚动容器引用，fitAll 用 */
  canvasEl = $state<HTMLElement | null>(null);
  /** 当前弹出的对话框：新建项目 / 设置 */
  dialog = $state<"new" | "settings" | null>(null);
  /** 设置弹窗打开时显示的标签页 */
  settingsTab = $state<"general" | "model" | "data" | "about">("general");
  /** 右栏抽屉：一次只开一个（board 宽 280 / agent 宽 440），null = 全收起 */
  drawer = $state<"board" | "agent" | null>("agent");
  /** 内置 Agent 对话（按项目持久化） */
  chat = $state<ChatEntry[]>([]);
  chatBusy = $state(false);
  /** 已请求停止，等后端回合收尾（按钮进入不可重复点击态） */
  chatStopping = $state(false);
  /** 当前使用的模型展示名（后端在回合开始时告知） */
  chatModel = $state("");
  /** 上下文用量（used / limit 均来自真实回执；limit 缺失 = 不显示环） */
  usage = $state<{ used: number; input: number; output: number; limit: number | null } | null>(null);
  /** 当前正在流式输出的助手条目 id（null = 下一个增量新起条目） */
  #streamingId: number | null = null;
  /** 模型接入配置是否就绪 */
  configured = $state(false);
  #chatId = 0;

  /** 打开设置弹窗（可指定标签页） */
  openSettings(tab: "general" | "model" | "data" | "about" = "general"): void {
    this.settingsTab = tab;
    this.dialog = "settings";
  }

  /** 打开项目：系统目录选择器选任意文件夹 → 挂回索引 → 当前窗口打开 */
  async openFromDisk(): Promise<void> {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const dir = await invoke<string | null>("pick_directory");
      if (!dir) return;
      const p = await projectService.importProject(dir);
      await projectService.open(p.name);
    } catch (e) {
      this.addLog(String(e).replace(/^.*Error:\s*/, "").trim() || "打开项目失败");
    }
  }

  /** 适应全部画板：按包围盒计算缩放并居中。force=true 时无视已 fit 标记（⌘1） */
  fitAll(force = false): void {
    const el = this.canvasEl;
    if (!el || this.loading || this.boards.length === 0) return;
    if (!force && this.fitted) return;
    const bs = this.boards;
    const minX = Math.min(...bs.map((b) => b.x));
    const minY = Math.min(...bs.map((b) => b.y));
    const maxX = Math.max(...bs.map((b) => b.x + b.width));
    const maxY = Math.max(...bs.map((b) => b.y + b.height));
    // 可用空间 = 画布视口减去四周 padding（padding 已含在 clientWidth 里，两侧都要减）
    const pad = 96 * 2;
    const availW = el.clientWidth - pad;
    const availH = el.clientHeight - pad;
    const z = Math.min(availW / (maxX - minX), availH / (maxY - minY), 1);
    this.zoom = Math.min(200, Math.max(20, Math.floor(z * 100)));
    this.fitted = true;
    // fit 后内容居中：取整产生的富余均匀分到两侧，而不是全堆在右边
    requestAnimationFrame(() => {
      el.scrollTo({
        left: (el.scrollWidth - el.clientWidth) / 2,
        top: (el.scrollHeight - el.clientHeight) / 2,
      });
    });
  }

  selected = $derived(this.boards.find((b) => b.id === this.selectedId) ?? null);
  searchText = $state("");
  filtered = $derived(
    this.searchText.trim()
      ? this.boards.filter((b) => b.name.toLowerCase().includes(this.searchText.trim().toLowerCase()))
      : this.boards,
  );

  async init() {
    this.project = await projectService.active();
    this.loadLogs();
    this.loadChat();
    this.refreshConfigured();
    await this.refresh();
  }

  /** 切换项目（project-opened 事件）：换加载对应项目的活动历史 */
  setProject(name: string) {
    if (this.project === name) return;
    this.project = name;
    this.loadLogs();
    this.loadChat();
  }

  /** 活动历史按项目落 localStorage：窗口重建/应用重启后仍在（此前只在内存，重启即清） */
  private logKey(): string {
    return `mlcanvas-activity-${this.project}`;
  }
  private loadLogs() {
    try {
      const raw = localStorage.getItem(this.logKey());
      this.logs = raw ? (JSON.parse(raw) as LogEntry[]) : [];
    } catch {
      this.logs = [];
    }
  }
  private saveLogs() {
    try {
      localStorage.setItem(this.logKey(), JSON.stringify(this.logs));
    } catch {
      // 存不下就算了（隐私模式/配额），活动流退化为仅本次会话
    }
  }

  async refresh(): Promise<void> {
    this.boards = await artboardService.list();
    if (this.selectedId && !this.boards.some((b) => b.id === this.selectedId)) {
      this.selectedId = null;
    }
    this.loading = false;
  }

  /** 画板的用户可见名：列表里取语义名，列表还没拿到时退回去后缀的文件名（UI 一律不暴露扩展名） */
  displayName(file: string): string {
    const board = this.boards.find((b) => b.id === file);
    return board?.name ?? file.replace(/\.html$/, "");
  }

  /** 文件变化：仅处理当前项目的，刷新并按画板精确重载 */
  async onChanged(changes: BoardChange[]): Promise<void> {
    const mine = changes.filter((c) => c.project === this.project);
    if (mine.length === 0) return;
    await this.refresh();
    const stamp = Date.now();
    const next = { ...this.versions };
    for (const c of mine) next[c.file] = stamp;
    this.versions = next;

    for (const c of mine) {
      this.addLog(
        (c.agent ? "Agent 更新了" : "更新了") + `「${this.displayName(c.file)}」画板`,
      );
    }
  }

  addLog(text: string): void {
    this.logs = [{ time: now(), text }, ...this.logs].slice(0, 20);
    this.saveLogs();
  }

  /** Agent 经 MCP 的动作 → 活动流文案（画板的写类由文件变化事件覆盖） */
  addAgentActivity(action: string, file: string | null): void {
    const name = file ? this.displayName(file) : null;
    const text =
      action === "list"
        ? "浏览了画板列表"
        : action === "read"
          ? `查看了「${name}」画板`
          : action === "query"
            ? `查询了「${name}」画板的元素`
            : action === "screenshot"
              ? `截取了「${name}」画板`
            : action === "skills"
              ? "浏览了设计技能库"
            : action === "skill"
              ? `查看了技能「${name}」`
              : action === "spec"
                ? "更新了风格规范"
                : `新增了组件「${name}」`;
    this.addLog(text);
  }

  // ── 内置 Agent 对话 ──

  private chatKey(): string {
    return `mlcanvas-chat-${this.project}`;
  }
  private loadChat() {
    try {
      const raw = localStorage.getItem(this.chatKey());
      const list = raw ? (JSON.parse(raw) as ChatEntry[]) : [];
      this.chat = list.map((e) => ({
        ...e,
        id: ++this.#chatId,
        streaming: false,
        toolState: e.kind === "tool" ? "done" : undefined,
      }));
    } catch {
      this.chat = [];
    }
  }
  private saveChat() {
    try {
      const list = this.chat.slice(-60).map(({ kind, text, error, compactCount }) => ({ kind, text, error, compactCount }));
      localStorage.setItem(this.chatKey(), JSON.stringify(list));
    } catch {
      // 存不下就算了（隐私模式/配额），对话退化为仅本次会话
    }
  }

  /** 模型接入配置是否就绪（init 与设置弹窗保存后刷新） */
  async refreshConfigured(): Promise<void> {
    try {
      const s = await agentService.getSettings();
      this.configured = s.configured;
    } catch {
      this.configured = false;
    }
  }

  private pushChat(entry: Omit<ChatEntry, "id">): void {
    this.chat = [...this.chat, { ...entry, id: ++this.#chatId }];
  }

  /** 助手回复的流式增量：追加到本轮正在输出的条目；工具步骤之后新起一条（每个回合分段显示） */
  onChatDelta(text: string): void {
    if (!this.chatBusy || !text) return;
    if (this.#streamingId === null) {
      const entry: ChatEntry = { id: ++this.#chatId, kind: "assistant", text, streaming: true };
      this.#streamingId = entry.id;
      this.chat = [...this.chat, entry];
      return;
    }
    this.chat = this.chat.map((e) =>
      e.id === this.#streamingId ? { ...e, text: e.text + text } : e,
    );
  }

  /** 工具步骤实时行：之前的 run 行收尾，追加新的 run 行；流式输出到工具调用即中断 */
  onChatTool(tool: string, file: string | null): void {
    this.chat = this.chat.map((e) =>
      e.kind === "tool" && e.toolState === "run" ? { ...e, toolState: "done" as const } : e,
    );
    this.#streamingId = null;
    const name = file ? this.displayName(file) : null;
    const text =
      tool === "get_conventions"
        ? "装载了设计手册"
        : tool === "get_project"
          ? "确认了工作目标"
          : tool === "list_artboards"
            ? "浏览了画板列表"
            : tool === "read_artboard"
              ? `查看了「${name}」画板`
              : tool === "register_artboard"
                ? `新建了「${name}」画板`
                : tool === "write_artboard"
                  ? `更新了「${name}」画板`
                  : tool === "delete_artboard"
                    ? `删除了「${name}」画板`
                    : tool === "rename_artboard"
                      ? "重命名了画板"
                      : tool === "notify_ready"
                        ? `刷新了「${name}」画板`
                        : tool === "screenshot_artboard"
                          ? `截取了「${name}」画板`
                          : tool === "query_element"
                            ? `检查了「${name}」画板的元素`
                            : tool === "get_design_spec"
                              ? "读取了风格规范"
                              : tool === "save_design_spec"
                                ? "更新了风格规范"
                                : tool === "save_component"
                                  ? "沉淀了组件"
                                  : "查阅了设计参考库";
    this.pushChat({ kind: "tool", text, toolState: "run" });
  }

  /** 发送一条消息并跑完一个回合（流式 + 工具循环都在后端） */
  async sendChat(text: string): Promise<void> {
    const trimmed = text.trim();
    if (!trimmed || this.chatBusy) return;
    this.drawer = "agent";
    this.chatBusy = true;
    this.chatStopping = false;
    this.#streamingId = null;
    this.pushChat({ kind: "user", text: trimmed });
    try {
      if (!this.configured) {
        throw new Error("尚未接入模型：点击下方「接入模型」连接你的模型服务");
      }
      // 历史只带文字（user/assistant），工具行是过程展示不进上下文
      const history = this.chat
        .filter((e) => (e.kind === "user" || e.kind === "assistant") && e.text.trim().length > 0 && !e.error)
        .slice(-50)
        .map((e) => ({ role: e.kind === "user" ? "user" : "assistant", content: e.text }));
      const final = await agentService.chatTurn(history);
      this.#finalizeStreaming(final);
    } catch (e) {
      const msg = String(e).replace(/^.*Error:\s*/, "").trim() || "对话失败，请重试";
      if (msg === "已停止") {
        // 停止不是错误：保留已经流出的内容，补一行中性收尾
        this.#endStreaming();
        this.pushChat({ kind: "step-end", text: "已停止" });
      } else if (this.#streamingId !== null) {
        // 已经流出过内容：错误作为单独一行附在后面
        this.#finalizeStreaming("");
        this.pushChat({ kind: "assistant", text: msg, error: true });
      } else {
        this.pushChat({ kind: "assistant", text: msg, error: true });
      }
    } finally {
      this.chatBusy = false;
      this.chatStopping = false;
      this.#streamingId = null;
      this.chat = this.chat.map((e) =>
        e.kind === "tool" && e.toolState === "run" ? { ...e, toolState: "done" as const } : e,
      );
      this.saveChat();
    }
  }

  /** 用回合最终文本替换正在流式的条目（以最终结果为准，防止丢增量） */
  #finalizeStreaming(final: string): void {
    if (this.#streamingId === null) return;
    this.chat = this.chat.map((e) =>
      e.id === this.#streamingId ? { ...e, text: final, streaming: false } : e,
    );
    this.#streamingId = null;
  }

  /** 收尾正在流式的条目：保留已有文本，只关掉光标 */
  #endStreaming(): void {
    if (this.#streamingId === null) return;
    this.chat = this.chat.map((e) =>
      e.id === this.#streamingId ? { ...e, streaming: false } : e,
    );
    this.#streamingId = null;
  }

  /** 上下文用量回执（每轮流式响应后送达） */
  onChatUsage(payload: { input: number; output: number; used: number; limit: number | null }): void {
    this.usage = payload;
  }

  /** 压缩上下文：把除最后两条外的历史整理成纪要，替换成一条可见的分隔记录 */
  async compactChat(): Promise<void> {
    if (this.chatBusy) return;
    const conversation = this.chat.filter(
      (e) => (e.kind === "user" || e.kind === "assistant") && e.text.trim() && !e.error,
    );
    if (conversation.length < 4) {
      this.pushChat({ kind: "step-end", text: "对话还短，不需要压缩" });
      return;
    }
    // 保留最后一问一答，其余交给模型整理
    const tail = conversation.slice(-2);
    const head = conversation.slice(0, -2);
    this.chatBusy = true;
    try {
      const { summary, count } = await agentService.compact(
        head.map((e) => ({ role: e.kind === "user" ? "user" : "assistant", content: e.text })),
      );
      const cut = new Set(head.map((e) => e.id));
      const rebuilt: ChatEntry[] = [
        {
          id: ++this.#chatId,
          kind: "compact",
          text: summary,
          compactCount: count,
        },
      ];
      for (const e of this.chat) {
        if (cut.has(e.id)) continue;
        if (tail.some((t) => t.id === e.id)) rebuilt.push(e);
      }
      this.chat = rebuilt;
      this.saveChat();
    } catch (e) {
      const msg = String(e).replace(/^.*Error:\s*/, "").trim() || "压缩失败，请重试";
      this.pushChat({ kind: "assistant", text: msg, error: true });
    } finally {
      this.chatBusy = false;
    }
  }

  /** 清空当前对话（画板与规范不受影响） */
  clearChat(): void {
    if (this.chatBusy) return;
    this.chat = [];
    this.usage = null;
    this.saveChat();
  }

  async cancelChat(): Promise<void> {
    if (!this.chatBusy || this.chatStopping) return;
    this.chatStopping = true;
    try {
      await agentService.cancel();
    } catch {
      // 取消是尽力而为：回合可能在收尾
    }
  }

  select(id: string, center = false): void {
    this.selectedId = id;
    if (center) this.centerId = id;
  }

  zoomIn(): void {
    this.zoom = Math.min(200, this.zoom + 10);
  }
  zoomOut(): void {
    this.zoom = Math.max(40, this.zoom - 10);
  }
  /** 滚轮等连续操作用细步进，不强制对齐 10 */
  zoomStep(delta: number): void {
    this.zoom = Math.min(200, Math.max(40, this.zoom + delta));
  }
  zoomReset(): void {
    this.zoom = 100;
  }

  togglePanel(panel: "design" | "resources" | "market"): void {
    if (this.panel === panel && this.panelOpen) {
      this.panelOpen = false;
    } else {
      this.panel = panel;
      this.panelOpen = true;
    }
  }

  /** 再点当前抽屉的按钮 = 收起；点另一个 = 直接切换（互斥） */
  toggleDrawer(which: "board" | "agent"): void {
    this.drawer = this.drawer === which ? null : which;
  }
}

export const designStore = new DesignStore();
