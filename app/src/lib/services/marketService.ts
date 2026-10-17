// 市场数据服务：列表走 vendor 期索引（一次读取，store 层缓存），详情按需读单包文件。
import { invoke } from "@tauri-apps/api/core";

export interface PackCover {
  bg: string;
  fg: string;
  mut: string;
  bd: string;
  btn: string;
  r: string;
}

export interface PackSummary {
  id: string;
  name: string;
  category: string;
  description: string;
  cover: PackCover;
  hasComponents: boolean;
}

export interface PackIndex {
  total: number;
  packs: PackSummary[];
}

export function listDesignSystems(): Promise<PackIndex> {
  return invoke<PackIndex>("list_design_systems");
}

export function readDesignSystem(name: string, file: string): Promise<{ name: string; file: string; content: string }> {
  return invoke("read_design_system", { name, file });
}
