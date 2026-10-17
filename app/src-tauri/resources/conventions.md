# 聿Canvas Agent 工作手册

> 这是 Agent 在聿Canvas 里工作的流程与规范全文——相当于本应用的 AGENTS.md。
> 会话开始先通读，创建或修改任何设计文件前遵守。
> 值只活在 tokens.css，法则只活在 design.md，两者零重叠。

## 工作流程

按以下顺序工作，不要跳步：

1. **确认工作目标** — 调用 `get_project` 拿到当前项目名与画板目录。所有工具都作用于应用的活动项目，不需要 cd 到任何目录；切换项目由用户在应用里完成，Agent 下次 `get_project` 自然感知。
2. **装载规范** — 调用 `get_conventions`（本文件）与 `list_skills` 了解可用的设计技能；做视觉设计前，按分类选读设计法则全文（`get_skill`，如 apple-design、emil-design-eng）。项目还没有风格规范（design.md / tokens.css）时，先与用户确认风格方向，或让 Agent 参考画布上已有画板提炼——风格由对话塑造，不要凭空假设。已有规范则用 `get_design_spec` 读入，全程遵守。
3. **生成画板** — 每个界面、方案变体、状态稿都是一块正式画板：`register_artboard` 登记（语义命名），`write_artboard` 写入内容。图标、组件、状态、菜单展开态等一切要"看一下、讨论一下"的东西都画成正式画板；不背着用户偷偷建删，废弃稿删除前确认。
4. **单板自检** — 每写完一块，立即 `screenshot_artboard` 看渲染结果，逐项核对：色彩是否符合规范、字阶与层级、间距节奏、有没有色差、内容是否溢出截断、交互逻辑是否合理、有没有违反设计技能的要求。发现违规就 `write_artboard` 改，改完再截图，确认无误再做下一块。
5. **全量复查** — 全部画板完成后，`list_artboards` 过一遍清单，逐块截图复查：跨画板的色彩/字阶/间距是否一致、有没有色差、有没有漏掉的交互逻辑或违反规范的地方、design.md 末节的画板索引是否与实际画板对账（新建未登记、删除未收缩都要修）。全部通过才算完成。
6. **交付** — 画板实时显示在用户画布上。向用户总结做了什么、每块画板的用途；改动后如未实时刷新，用 `notify_ready` 兜底。

**工具排他纪律**：渲染、截图、元素查询一律用本工具集（`screenshot_artboard` / `query_element`）——它们与用户画布是同一条渲染管线，所见即用户所见；应用外的浏览器、脚本截图看到的是别的渲染面，不可作为依据。修改项目内文件一律走 MCP 工具（`write_artboard` / `save_design_spec` / `save_component`），不要直接改磁盘——走工具才有活动归属与实时刷新。

## 工具使用指导

| 场景 | 工具 | 用法要点 |
|---|---|---|
| 会话起点 | `get_project` | 第一个调用的工具；返回当前项目名与路径，确认工作目标 |
| 看现状 | `list_artboards` | 画板清单（文件名/尺寸/位置/标题）；动手前先看有哪些板，交付前用它做全量复查 |
| 读画板 | `read_artboard` | 返回画板 HTML 全文；改前先读，避免整页覆盖丢掉既有内容 |
| 新建画板 | `register_artboard` | 登记 → 拿到文件名 → 再 `write_artboard` 写内容；语义命名（`<title>` 中文，文件名自动生成随机后缀） |
| 写画板 | `write_artboard` | 整页 HTML/CSS/JS，禁框架与 TS；画板是静态设计图，状态画出来不模拟；写完必须 `screenshot_artboard` 自检 |
| 改名/删除 | `rename_artboard` / `delete_artboard` | 改的是 `<title>` 语义名（文件名不动）；删除是废弃稿清理，先与用户确认；两者都要同步 design.md 画板索引 |
| 自检 | `screenshot_artboard` | 返回当前渲染 PNG；每次写入/修改后必调，按工作流程第 4/5 步清单核对 |
| 定位元素 | `query_element` | 查画板视口内某坐标处的元素（标签/类名/文本/位置）；确认"这块是什么"用 |
| 通知刷新 | `notify_ready` | 写入后画板自动刷新，此工具是兜底；每次修改后调用 |
| 读规范 | `get_design_spec` | 返回项目 design.md + tokens.css 全文；设计前读，全程遵守 |
| 写规范 | `save_design_spec` | 建立或更新项目风格；注意回执软提示（未登记中文名、design.md 出现色值等），收到就修 |
| 沉淀组件 | `save_component` | 从画板抽离可复用组件；mc- BEM 命名、样式自包含、取值 var(--令牌, 兜底) |
| 技能库 | `list_skills` / `get_skill` | 内置设计技能参考库；list 出分类清单（指针卡无正文），做视觉设计前按分类选读全文 |
| 风格包 | `list_design_systems` / `get_design_system` | 内置风格包（Linear/Notion/Stripe 等实物参考）；包是只读材料，应用 = 读包 → 把令牌翻译成本项目命名约定 → `save_design_spec` 写入 |

## 画板格式

- 文件内容是**纯 HTML/CSS/JS**，禁止引入框架、TypeScript、外部依赖。
- 首个 `<meta>` 声明画布几何：`<meta name="artboard" content="320x640, x=360, y=0">`。宽高必填；x/y 可省略，省略的画板由工具自动排在已声明画板包围盒右侧。
- `<title>` 用中文语义名（如「首页」），它是用户在界面里看到的画板名。
- 文件名 = 语义名小写 + 4 位随机十六进制（如 `home-9f3a.html`），由 `register_artboard` 生成，**永不复用已删除画板的文件名**；废弃的画板在 design.md 画板索引里标「已废弃」而不是删文件。
- 位置和间距是 Agent 的设计职责：声明 x/y 时避免与其他画板重叠。
- 画板可引用项目内相对资源：`<link rel="stylesheet" href="../tokens.css">`、`../components/…`、本地图片——渲染管线会解析相对路径；令牌改变全项目画板跟随，不要把值内联进画板。

## tokens.css 命名规范

语法：`--<类别>-<名称>`，变体 `--<类别>-<名称>-<修饰>`。类别前缀固定英文，**名称强制英文**。

八类词汇表：

| 类别 | 形式 | 说明 |
|---|---|---|
| color | `--color-<角色>` | 推荐角色：primary、primary-foreground、background、foreground、muted、border、card、success、warning、danger |
| font-size | `--font-size-<角色>` | 与 font-weight 同名配对成字阶；角色：display、heading、body、label、caption |
| font-weight | `--font-weight-<角色>` | 与同名 font-size 配对 |
| space | `--space-<N>` | N 按 4px 步进（4/8/12/16/24…） |
| radius | `--radius-<名称>` | 圆角 |
| shadow | `--shadow-<名称>` | 投影 |
| font-family | `--font-family-<名称>` | 字族 |
| leading / motion / ease | `--leading-<名称>` 等 | 行高与动效曲线 |

硬性约束：色彩值必须是字面值（十六进制/rgb），**禁止 var() 引用**；全部令牌必须在 design.md「令牌对照」登记中文名。

## design.md 格式宪法

固定结构，章节名不得更改：

1. `# 风格名` + 一段引言
2. `## 令牌对照` — 每行一条 `- --完整令牌名：中文名`，这是 tokens.css 全部令牌的中英词典，tokens.css 里的每个令牌都必须登记
3. `## 视觉主题` / `## 设计原则` / `## 组件规则` / `## 动效` / `## 文案口吻` / `## 设计禁令`
4. 末节 `## 画板索引` — 每行一条 `- <文件名>：<用途描述>；状态（定稿/草稿/已废弃/参考）`

禁令：design.md **禁止出现任何具体值**（hex 色值、px 刻度）——引用一律用令牌名。

## mc- 组件规范

- 类名 BEM：`mc-<组件>` 根类、`mc-<组件>--<修饰>` 变体、`mc-<组件>__<子元素>`。
- 文件名与根类同名（英文 slug），`<title>` 中文。
- 样式自包含（组件内 `<style>`），取值一律 `var(--令牌, 兜底值)`。
