import { invoke, convertFileSrc } from "@tauri-apps/api/core";

/** 后端返回的画板条目 */
export interface Artboard {
  id: string;
  /** 显示名称（取自画板 title），UI 一律用名称 */
  name: string;
  file_name: string;
  path: string;
  /** 画板声明的固定尺寸（CSS 像素） */
  width: number;
  height: number;
  /** 画布位置；未在画板 HTML 声明时由后端自动平铺 */
  x: number;
  y: number;
  /** asset 协议地址，iframe 直接加载 */
  url: string;
}

/**
 * 画板服务：封装与 Rust 后端的 IPC 交互。
 * 后端返回画板的绝对路径，这里负责转成 WebView 可加载的 asset 协议地址。
 */
export const artboardService = {
  async list(): Promise<Artboard[]> {
    const boards = await invoke<Artboard[]>("list_artboards");
    return boards.map((b) => ({ ...b, url: convertFileSrc(b.path) }));
  },
  register(file_name: string, title?: string): Promise<string> {
    return invoke<string>("register_artboard", { file_name, title });
  },
  read(file_name: string): Promise<string> {
    return invoke<string>("read_artboard", { file_name });
  },
  write(file_name: string, content: string): Promise<string> {
    return invoke<string>("write_artboard", { file_name, content });
  },
};
