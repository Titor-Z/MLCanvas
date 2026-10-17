import { invoke } from "@tauri-apps/api/core";

/** Agent 接入状态：只基于真实信号——MCP initialize 的 clientInfo（接入身份）与调用进出（工作中） */
class AgentPresence {
  /** 最近接入的 MCP 客户端名（如 zcode），空 = 本次启动尚无客户端接入 */
  client = $state("");
  /** 是否有调用正在执行 */
  busy = $state(false);

  /** 展示名：首字母大写（zcode → ZCode） */
  name = $derived(this.client ? this.client.charAt(0).toUpperCase() + this.client.slice(1) : "");

  /** 状态灯文案与色调：工作中（蓝）/ 可用（绿）/ 未接入（灰） */
  display = $derived(
    this.busy
      ? { text: this.name ? `${this.name} 工作中` : "Agent 工作中", tone: "busy" as const }
      : this.client
        ? { text: `${this.name} 可用`, tone: "ok" as const }
        : { text: "Agent 未接入", tone: "off" as const },
  );

  setClient(name: string): void {
    this.client = name;
  }

  setBusy(busy: boolean): void {
    this.busy = busy;
  }

  /** 窗口挂载时查询常驻接入状态（接入是事实不是事件，晚开的窗口也要拿到） */
  async init(): Promise<void> {
    try {
      const client = await invoke<string>("agent_client");
      if (client) this.client = client;
    } catch {
      // 无 Tauri 后端的浏览器调试环境
    }
  }
}

export const agentPresence = new AgentPresence();
