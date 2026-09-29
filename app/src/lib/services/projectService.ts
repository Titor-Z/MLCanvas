import { invoke } from "@tauri-apps/api/core";

export interface ProjectInfo {
  name: string;
  artboards: number;
  updated_at: number;
}

export const projectService = {
  list(): Promise<ProjectInfo[]> {
    return invoke<ProjectInfo[]>("list_projects");
  },
  create(name: string): Promise<ProjectInfo> {
    return invoke<ProjectInfo>("create_project", { name });
  },
  open(name: string): Promise<void> {
    return invoke("open_project", { name });
  },
  active(): Promise<string> {
    return invoke<string>("get_active_project");
  },
  agentConnected(): Promise<boolean> {
    return invoke<boolean>("bridge_status");
  },
};
