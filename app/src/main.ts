import { mount } from "svelte";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./styles.css";
import HomeApp from "./home/HomeApp.svelte";
import DesignApp from "./design/DesignApp.svelte";

// 按窗口标签路由：main = 主页小窗，design = 设计主窗；无 Tauri 环境回退主页，#design 为调试通道
const isDesign = (() => {
  try {
    return getCurrentWindow().label === "design" || location.hash === "#design";
  } catch {
    return location.hash === "#design";
  }
})();
mount(isDesign ? DesignApp : HomeApp, { target: document.getElementById("app")! });
