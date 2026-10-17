import { invoke } from "@tauri-apps/api/core";

/** 一份模型配置（键名与 Rust 侧一致） */
export interface ModelConfig {
  id: string;
  provider: string;
  protocol: string;
  base_url: string;
  api_key: string;
  model: string;
}

/** 后端返回的全局设置（多份模型配置 + 当前使用 + 默认项目位置） */
export interface AgentSettingsInfo {
  current: string;
  configs: ModelConfig[];
  default_parent: string;
  /** 是否已配置到能发起对话的最小信息 */
  configured: boolean;
  version: string;
}

/** 新增/更新配置的入参（id 省略 = 新增） */
export interface ModelConfigInput {
  id?: string;
  provider: string;
  protocol: string;
  base_url: string;
  api_key: string;
  model: string;
}

/** 公共模型库（models.dev）里的一条模型 */
export interface ModelCatalogItem {
  id: string;
  name: string;
  context: number;
  reasoning: boolean;
  vision: boolean;
}

export interface ModelCatalog {
  models: ModelCatalogItem[];
  source: string;
}

/**
 * 内置 Agent 服务：全局设置、模型配置 CRUD、连接测试、对话回合。
 * 对话的流式增量与工具步骤经事件 agent-chat-delta / agent-chat-tool 推送。
 */
export const agentService = {
  getSettings(): Promise<AgentSettingsInfo> {
    return invoke<AgentSettingsInfo>("agent_get_settings");
  },
  /** 新增/更新一份配置，返回配置 id */
  saveConfig(input: ModelConfigInput): Promise<string> {
    return invoke<string>("agent_save_config", {
      id: input.id ?? null,
      provider: input.provider,
      protocol: input.protocol,
      baseUrl: input.base_url,
      apiKey: input.api_key,
      model: input.model,
    });
  },
  deleteConfig(id: string): Promise<void> {
    return invoke("agent_delete_config", { id });
  },
  setCurrent(id: string): Promise<void> {
    return invoke("agent_set_current", { id });
  },
  /** 新建项目默认父目录；空串 = 恢复默认 */
  setDefaultParent(dir: string): Promise<void> {
    return invoke("agent_set_default_parent", { dir });
  },
  /** 按接口地址查公共模型库 */
  modelCatalog(baseUrl: string): Promise<ModelCatalog> {
    return invoke<ModelCatalog>("agent_model_catalog", { baseUrl });
  },
  /** 发一条最小请求验证配置；失败抛出可读的错误消息 */
  testConnection(input: Omit<ModelConfigInput, "id" | "provider">): Promise<void> {
    return invoke("agent_test_connection", {
      protocol: input.protocol,
      baseUrl: input.base_url,
      apiKey: input.api_key,
      model: input.model,
    });
  },
  /** 发起一个对话回合（含工具调用循环），返回助手最终回复全文 */
  chatTurn(messages: { role: string; content: string }[]): Promise<string> {
    return invoke<string>("agent_chat_turn", { messages });
  },
  /** 请求停止当前回合（在工具步骤间生效） */
  cancel(): Promise<void> {
    return invoke("agent_chat_cancel");
  },
  /** 压缩上下文：模型把前段历史整理成工作纪要，返回摘要与消耗的条目数 */
  compact(messages: { role: string; content: string }[]): Promise<{ summary: string; count: number }> {
    return invoke("agent_chat_compact", { messages });
  },
};
