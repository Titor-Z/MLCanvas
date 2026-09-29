# AGENTS.md
> AI 开发前必须阅读的手册

## 介绍
- 项目名叫 MLCanvas
- 项目UI使用 Vite + Svelte来做。
- MCP：提供工具接口 + Skill，每个agent接入mcp，不用单独安装skill就能直接上手
- 定位：Agent 优先的UI设计工具。WEB即是一切。借鉴OpenDesign的定位和功能设计。

## 商定
- 壳 UI 用 Vite+Svelte+TypeScript，画板内容禁用框架和 TS，只允许纯 HTML/CSS/JS。
- why.md 是 AI 和用户的讨论记录，只作背景参考，不是唯一实现指南；规范以本文档商定为准。
- 画板是固定尺寸的画布，不是自适应容器：尺寸由画板 HTML 里 `<meta name="artboard" content="宽x高">` 声明（缺省 320×640），不随应用窗口大小缩放；窗口变化只影响画板的排布和滚动。
- 画板的渲染/截图/元素查询坐标系都以画板自身尺寸为准。
- 界面只说设计语言，禁止暴露技术细节：任何用户可见的 UI 文案不得出现文件名、扩展名（.html）、路径、端口、协议、监听机制等术语。画板用语义名称（来自画板 `<title>`，如「首页」），文件名等技术信息只允许出现在专门的调试视图中。
- 画板标签和画布上只显示名称，不显示尺寸/技术数字；尺寸等属性只出现在属性检查器里。标题栏按文档标题设计（项目名居中），不放技术状态。
- 不做开发者状态栏：刷新时间、文件监听、服务地址等运行时状态一律不进 UI。界面展示的是设计内容，不是程序日志。

## 开发规范
- 使用MVC + OOP 面向对象编程 + 设计模式拆分功能 和 模块，保持高内聚低耦合
- 无用的代码直接删除，不要留死代码。
- 不要偷懒，是直接删原来的代码，重新写，不要包裹旧代码
- 一个问题重复3次解决不了，立即停止，和用户协商
- 本地已有 `tgrep` 指令，可以替代grep的使用，属于grep的增强，请优先使用 __`tgrep`__
- 本地已经有 `gh` 工具，可直接登录github
- github 资源空间和免费时长有限，如需 `github actions`，使用 tag 触发，不要每次提交都触发，
- `github action` 同时必须要做到超过15分钟编译时常的话，就停止它，肯定是有死循环代码。另外，新的actions开始，必须先停掉正在运行的旧的 actions。
- 本地磁盘空间有限，编译时要考虑到。
- cargo check 的时候，不要每次都全局，cargo build 的时候，也不要每次都编译全局，最小化测试和编译和检测，就一切可能降低内存，并缩短时间
- AI 停止废话，停止幻觉，不要问太多，优先回答和本项目有关的事


## 讨论
> 这里存放用户和AI的对话记录，用陈述句总结，并提取其中的主要内容和思路方向，汇总后写入。

- 定稿了文档冲突：壳 UI 用 Vite+Svelte+TypeScript，画板内容只允许纯 HTML/CSS/JS；项目定名 MLCanvas。
- V1 最小链路（渲染+监听+刷新）已跑通；MCP 已接入，7 个工具（读写画板/截图/元素查询）端到端测试通过。
- 下一步方向：V2 元素审查与标注（点击画板定位源码）、组件库 components/ 与 Skill 规范。
- UI 设计定稿方向：壳 UI 按最终版本形态设计（不按 V1 凑合），参照 Adobe XD 的简洁直接。稿子：artboards/ui-home.html（主页）+ artboards/ui-final.html（设计模式）。
- 从 Adobe XD 实机截图提炼并采纳：暗色主题（用户选定）；标题栏合并菜单栏（文件/编辑/视图/画板/Agent/帮助）+ 居中文档名 + 缩放比在标题栏右上（不用画布浮块）；有"主页"概念（最近项目 + 创建入口）；检查器用成组可编辑输入框（名称/尺寸 W×H）。MLCanvas 的差异点：主页的核心创建动作是"向 Agent 描述界面"；检查器有 Agent 协作节；状态栏报告选中对象与 Agent 状态。
- 左侧结构定稿：最左窄工具栏（上：选择/抓手/标注；下：3 个面板切换图标），左面板可切换三块内容——设计（画板列表，默认）、组件（设计稿抽离的组件效果预览）、规范（design.md 风格的 token 展示，形态待定）；点击图标切换面板，再点当前图标关闭面板。
- 对话架构定稿：对话在 Agent 侧（用户自己的 agent 客户端），MLCanvas 通过 MCP 与 Agent 通信，工具内不做聊天；工具内的反馈形态是标注/便签（V2），经 MCP 供 Agent 读取。主页不放对话入口，职责是最近项目 + 新建项目 + 工作流引导。
- 项目概念定稿：用户创建的是「项目」（项目网格首个虚线方块，Apple Files/Notion 范式），画板是 Agent 在对话中生成的工作产物，用户不手动新建画板；主页不放设备尺寸选择。主页 960×640 独立小窗，无左导航栏（内容列 640px 居中，设置在标题栏），最近打开支持 方块/列表 视图切换（稿：ui-home.html / ui-home-list.html），样式遵循设计 token 体系（字阶 24/14/13/11、4px 间距网格、text-3=#86868b 保证对比度）。
- 新建项目流程定稿：主页弹轻量模态（非页面），只需项目名称，创建后打开项目画布并引导用户在 Agent 中描述第一个界面；不问路径（默认存储）、不问尺寸（Agent 对话中定）。稿：ui-home-new.html。
- 标题栏完全自绘（菜单 + 最小化/最大化/关闭三键固定右上），三平台一套；拖拽区、双击最大化等平台细节用 Tauri API 自行实现。
- 不设底部状态栏：状态信息的归属是它的语境——选中对象在检查器、计数在导航栏、Agent 状态在 Agent 协作区标题行。
- 每次做 UI 设计之前，必须先通读设计类 Skill 的设计法则（apple-design、emil-design-eng 等），遵守后再动手。
- 信息架构定稿：标题栏自绘跨平台统一（窗口控制 + 菜单栏 + 居中文档名）；左侧=导航器（工具 + 画板列表 + 组件库 + 设计规范）；右侧=检查器（画板属性 + 显示 + Agent 协作/活动流/指令输入）；底部状态栏=选中对象 + Agent 状态 + 画板数；画布右下浮动缩放控件。
- 0.2.0 已实现：项目体系（~/MLCanvas/<名称>/artboards/，首启建示例项目并迁移旧 artboards）、主页 960×640（问候 + 项目网格/列表 + 新建模态）、设计窗口 1280×800（动态创建，标题栏菜单/居中文档名/缩放、工具栏、三面板切换、画布、检查器）。watcher 载荷升级 {project, file}，MCP 走活动项目。验证：前端构建通过、MCP 三工具（list/screenshot/query）回归通过、主页与设计模式浏览器渲染视觉审查通过。已知限制：单活动项目。
- 画布交互定稿：缩放数值可点击输入（钳制 40–200%、± 步进 10、滚轮细步进 5）；⌘/Ctrl/Alt+滚轮缩放；左侧画板列表点选后画板滚动到视口中央（画布内点选不触发）；抓手 = 左侧工具常驻 + 按住空格临时切换（Figma/Sketch 惯例，光标 grab/grabbing，抓手激活时 iframe pointer-events 放行以便整画布拖拽）。
- 画布布局定稿：自由画布（Figma 范式）——画板按声明坐标绝对定位，允许负坐标，fit-all 按包围盒计算。位置由 Agent 声明：`<meta name="artboard" content="320x640, x=360, y=0">`（x/y 可省略，省略的画板由工具自动排在已声明画板包围盒右侧一条基线上）。Agent 约束：位置和间距是 Agent 的设计职责，避免重叠；不声明位置也能工作（自动平铺兜底）。
- 已知边界：画板 iframe 是可交互的独立文档，⌘+滚轮在其上方会被 iframe 吃掉（空隙处正常）；彻底解决要等 V2 元素审查的事件桥。抓手拖拽已用 pointer-events 方案绕过同类问题。

## 认知修正
> 这里存放我们遇到的问题，和解决方法，用知识点的陈述句来写。防止AI再次犯错。

- Tauri 的 asset:// 页面对 IPC 而言是 Remote origin：应用自定义命令会走 ACL 被拒；插件命令（如事件 emit_to）也需要 capability 里显式声明 `remote.urls`，且匹配的是不带路径的 origin（`asset://localhost`，不是 `asset://localhost/*`）。
- Tauri 的 `AppHandle::listen` 注册的是 App target 监听器，收不到 JS `emit_to(target: Any)` 的事件；要用 `listen_any`。
- macOS 上 WKWebView 的 document.title 不会同步到 NSWindow 标题，调试网页脚本要用 `location.hash` + `WebviewWindow::url()` 做诊断通道。
- tauri/wry 没有公开的网页截图 API；进程内截图走 `with_webview` 拿 WKWebView 指针 + objc2 takeSnapshot，再经 NSBitmapImageRep 编码 PNG。
- objc2 绑定：AppKit 多数类是 AnyThread（`alloc()` 需要 `objc2::AnyThread` trait 在作用域），WKWebView 是 MainThreadOnly；init 方法是关联函数形式（首参 `Allocated<Self>`），不能链式调用。
- npm 缓存目录 ~/.npm 有 root 属主文件时，用 `npm install --cache /tmp/xxx` 绕开，避免依赖 sudo。
- Svelte 5 可变状态必须 `let ... = $state()`，`const` 声明不可赋值也不可 bind；`bind:this` 目标同样要 `let`。
- 无 Tauri 后端的普通浏览器里 `getCurrentWindow()` 直接抛错（无 `__TAURI_INTERNALS__`），会中断组件渲染；壳代码取窗口对象要 try/catch 兜底，路由用 `location.hash === "#design"` 作为调试通道。
- 终端没有「屏幕录制」权限时，`screencapture` 截到的桌面会缺其他应用的窗口（只剩壁纸+菜单栏），按窗口 ID 截图直接失败——验证应用窗口渲染应走无头/内嵌浏览器，而不是 screencapture。
- Tauri CLI 运行时会按 tauri.conf.json 自动增删 Cargo.toml 里 tauri 依赖的 features：手动加 feature 而配置没声明，会被 CLI 删回去（表现为"改了又还原"）。透明窗口的正确做法：配置 `app.macOSPrivateApi: true`（OS 大写）+ 窗口 `transparent: true`，feature 由 CLI 自己写入。
- 无边框窗口（decorations:false）是直角的；圆角靠 transparent + 内容容器 border-radius + overflow hidden 实现。但本机 macOS 12 上透明 WKWebView 窗口合成不完整（内容下半部分消失、甚至整窗不可见），已放弃圆角、恢复不透明直角窗口。另外 Tauri 2 的窗口控制（拖动/最小化/最大化/关闭）不在 core:default 权限集里，需在 capability 显式声明 core:window:allow-*。
- WebKit 滚动容器（overflow:auto）的右/下 padding 不计入滚动区域：滚到尽头时内容直接贴边。绕法：留白不做容器 padding，做进内容盒自身的宽高里（如画布 flow 尺寸 +192，绝对定位子元素坐标 +96）。margin 同样不可靠。
- 无屏幕录制权限时无法用 screencapture 看到应用窗口，但应用进程内可以：Inspector 的 WKWebView takeSnapshot 已扩展为 debug_shell_screenshot（任意窗口）和 debug_shell_eval（窗口内执行 JS）两个 MCP 调试工具，壳 UI 问题可自测自证，不必依赖用户截图。

## 其他资源
[why.md](./why.md) 项目基础定位，只用来看，不要用来修改，如需修改，在本文档中做引用和修改。
[CHANGELOG](./CHANGELOG) 存放我们的版本更新记录
