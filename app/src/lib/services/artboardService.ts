import { invoke } from "@tauri-apps/api/core";

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
  /** 画板页面地址（资源服务直出，相对路径可解析），由后端拼接 */
  url: string;
  /** 画板 HTML 是否显式声明了位置 */
  declared_pos: boolean;
}

/**
 * 画板服务：封装与 Rust 后端的 IPC 交互。
 * 画板地址由后端直出（本机资源服务，相对路径可解析）；前端不再转换。
 */
export const artboardService = {
  async list(): Promise<Artboard[]> {
    return invoke<Artboard[]>("list_artboards");
  },
  register(file_name: string, title?: string): Promise<string> {
    return invoke<string>("register_artboard", { fileName: file_name, title: title ?? null });
  },
  /** 删除画板（文件一并移除，废弃稿清理） */
  remove(file_name: string): Promise<void> {
    return invoke("delete_artboard", { fileName: file_name });
  },
  /** 修改画板标题（用户可见的语义名称，即 <title>；文件名不动） */
  setTitle(file_name: string, title: string): Promise<void> {
    return invoke("set_artboard_title", { fileName: file_name, title });
  },
  /** 保存画板排序（画布布局随之单行重排） */
  saveOrder(files: string[]): Promise<void> {
    return invoke("save_artboard_order", { files });
  },
  read(file_name: string): Promise<string> {
    return invoke<string>("read_artboard", { fileName: file_name });
  },
  write(file_name: string, content: string): Promise<string> {
    return invoke<string>("write_artboard", { fileName: file_name, content });
  },
};
