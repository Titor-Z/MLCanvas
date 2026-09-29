import { artboardService, type Artboard } from "../lib/services/artboardService.js";
import { projectService } from "../lib/services/projectService.js";

export interface BoardChange {
  project: string;
  file: string;
}

export interface LogEntry {
  time: string;
  text: string;
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
  panel = $state<"design" | "components" | "spec">("design");
  panelOpen = $state(true);
  inspectorOpen = $state(true);
  zoom = $state(100);
  /** 打开项目后首次 fit-all 已执行，切换项目时复位 */
  fitted = $state(false);
  showLabels = $state(true);
  logs = $state<LogEntry[]>([]);
  loading = $state(true);
  /** 画板列表点击时置为目标画板，画布收到后将其滚动到视口中央 */
  centerId = $state("");

  selected = $derived(this.boards.find((b) => b.id === this.selectedId) ?? null);
  searchText = $state("");
  filtered = $derived(
    this.searchText.trim()
      ? this.boards.filter((b) => b.name.toLowerCase().includes(this.searchText.trim().toLowerCase()))
      : this.boards,
  );

  async init() {
    this.project = await projectService.active();
    await this.refresh();
  }

  async refresh(): Promise<void> {
    this.boards = await artboardService.list();
    if (this.selectedId && !this.boards.some((b) => b.id === this.selectedId)) {
      this.selectedId = null;
    }
    this.loading = false;
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
      const board = this.boards.find((b) => b.id === c.file);
      this.addLog(`更新了「${board ? board.name : c.file}」`);
    }
  }

  addLog(text: string): void {
    this.logs = [{ time: now(), text }, ...this.logs].slice(0, 20);
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

  togglePanel(panel: "design" | "components" | "spec"): void {
    if (this.panel === panel && this.panelOpen) {
      this.panelOpen = false;
    } else {
      this.panel = panel;
      this.panelOpen = true;
    }
  }

  toggleInspector(): void {
    this.inspectorOpen = !this.inspectorOpen;
  }
}

export const designStore = new DesignStore();
