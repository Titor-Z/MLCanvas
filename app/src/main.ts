import { mount } from "svelte";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import "./styles.css";
import HomeApp from "./home/HomeApp.svelte";
import DesignApp from "./design/DesignApp.svelte";

// 启动期 JS 错误经 debug_log 命令落应用 stderr，供无控制台环境诊断
window.addEventListener("error", (e) => {
  invoke("debug_log", { msg: `[${location.pathname}] ${e.message || e.error}` }).catch(() => {});
});

// 按窗口标签路由：main = 主页小窗，design = 设计主窗；无 Tauri 环境回退主页，#design 为调试通道
const isDesign = (() => {
  try {
    return getCurrentWindow().label === "design" || location.hash === "#design";
  } catch {
    return location.hash === "#design";
  }
})();
mount(isDesign ? DesignApp : HomeApp, { target: document.getElementById("app")! });
