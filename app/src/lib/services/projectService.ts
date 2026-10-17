import { invoke } from "@tauri-apps/api/core";

export interface ProjectInfo {
  name: string;
  /** 项目文件夹绝对路径（显示时把用户目录前缀缩短为 ~） */
  path: string;
  artboards: number;
  updated_at: number;
}

export const projectService = {
  list(): Promise<ProjectInfo[]> {
    return invoke<ProjectInfo[]>("list_projects");
  },
  create(name: string, dir?: string): Promise<ProjectInfo> {
    return invoke<ProjectInfo>("create_project", { name, dir: dir ?? null });
  },
  /** 打开项目：选择 ~/YuCanvas 下的现有文件夹，挂回索引 */
  importProject(dir: string): Promise<ProjectInfo> {
    return invoke<ProjectInfo>("import_project", { dir });
  },
  open(name: string): Promise<void> {
    return invoke("open_project", { name });
  },
  remove(name: string): Promise<void> {
    return invoke("delete_project", { name });
  },
  active(): Promise<string> {
    return invoke<string>("get_active_project");
  },
};

/** /Users/<name>/... → ~/...，用于界面展示 */
export function shortenPath(path: string): string {
  return path.replace(/^\/Users\/[^/]+/, "~");
}
