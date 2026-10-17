import { invoke } from "@tauri-apps/api/core";

/** 后端返回的风格规范原文（design.md + tokens.css，均可能为空串） */
export interface DesignSpecRaw {
  name: string;
  design_md: string;
  tokens_css: string;
}

export interface Swatch {
  name: string;
  /** design.md「令牌对照」登记的中文名；未登记时界面回退英文名 */
  label?: string;
  value: string;
}

export interface TypeItem {
  name: string;
  label?: string;
  size: number;
  weight: number;
}

export interface SpaceItem {
  name: string;
  label?: string;
  value: number;
}

export interface RadiusItem {
  name: string;
  label?: string;
  value: number;
}

export interface ShadowItem {
  name: string;
  label?: string;
  value: string;
}

/** 规范的画廊视图：资源面板直接渲染的结构 */
export interface SpecView {
  name: string;
  colors: Swatch[];
  types: TypeItem[];
  spaces: SpaceItem[];
  radii: RadiusItem[];
  shadows: ShadowItem[];
  bans: string[];
}

export interface ComponentInfo {
  file_name: string;
  name: string;
  source: string;
  path: string;
  html: string;
}

/** 资源服务：风格规范（项目的品牌契约）+ 组件（从画板抽离的实物） */
export const specService = {
  async get(): Promise<DesignSpecRaw> {
    return invoke<DesignSpecRaw>("get_design_spec");
  },
  async components(): Promise<ComponentInfo[]> {
    return invoke<ComponentInfo[]>("list_components");
  },
};

/**
 * 把 tokens.css + design.md 解析成画廊视图。
 * 令牌命名约定（MCP save_design_spec 已声明）：--<类别>-<名称>（类别固定英文，名称强制英文），
 * 面板消费 --color-*、--font-size-* + --font-weight-*（同名配对）、--space-*、--radius-*、--shadow-*，
 * 其余类别合法但暂不渲染；
 * design.md 首节「令牌对照」用 `- --完整令牌名：中文名` 给令牌登记中文名，未登记的回退英文；
 * 「## 设计禁令」节用 - 列表逐条写红线。
 */
export function parseSpec(raw: DesignSpecRaw): SpecView {
  const colors: Swatch[] = [];
  const types: TypeItem[] = [];
  const spaces: SpaceItem[] = [];
  const radii: RadiusItem[] = [];
  const shadows: ShadowItem[] = [];
  // 宽匹配而非 \w：名称是非 ASCII 时 \w 会整组漏掉（中文令牌时代的教训）
  const re = /--([^\s:;]+)\s*:\s*([^;]+);/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(raw.tokens_css))) {
    const key = m[1];
    const value = m[2].trim();
    if (key.startsWith("color-")) {
      colors.push({ name: key.slice(6), value });
    } else if (key.startsWith("font-size-")) {
      types.push({ name: key.slice(10), size: parseFloat(value) || 0, weight: 400 });
    } else if (key.startsWith("font-weight-")) {
      const name = key.slice(12);
      const weight = parseFloat(value) || 400;
      const existing = types.find((t) => t.name === name);
      if (existing) existing.weight = weight;
      else types.push({ name, size: 0, weight });
    } else if (key.startsWith("space-")) {
      spaces.push({ name: key.slice(6), value: parseFloat(value) || 0 });
    } else if (key.startsWith("radius-")) {
      radii.push({ name: key.slice(7), value: parseFloat(value) || 0 });
    } else if (key.startsWith("shadow-")) {
      shadows.push({ name: key.slice(7), value });
    }
  }
  types.sort((a, b) => b.size - a.size);
  spaces.sort((a, b) => b.value - a.value);
  radii.sort((a, b) => a.value - b.value);

  // design.md 的「令牌对照」与「设计禁令」两节
  const labels: Record<string, string> = {};
  const bans: string[] = [];
  let section = "";
  for (const line of raw.design_md.split("\n")) {
    const h = line.trim();
    if (h.startsWith("## ")) {
      const title = h.slice(3).trim();
      section = title === "令牌对照" ? "labels" : title.includes("禁令") ? "bans" : "";
      continue;
    }
    if (!h.startsWith("- ")) continue;
    if (section === "bans") {
      bans.push(h.slice(2).trim());
    } else if (section === "labels") {
      const item = /^-\s*(--[^\s:：]+)\s*[:：]\s*(.+)$/.exec(h);
      if (item) labels[item[1]] = item[2].trim();
    }
  }
  for (const c of colors) c.label = labels[`--color-${c.name}`];
  for (const t of types) t.label = labels[`--font-size-${t.name}`];
  for (const s of spaces) s.label = labels[`--space-${s.name}`];
  for (const r of radii) r.label = labels[`--radius-${r.name}`];
  for (const s of shadows) s.label = labels[`--shadow-${s.name}`];

  return { name: raw.name, colors, types, spaces, radii, shadows, bans };
}

/**
 * 组件片段 → iframe 预览文档。
 * 布局：去默认边距、透明底、body 居中——组件在 tile 里居中呈现（对齐设计稿）；
 * 适配：组件超出视口时等比缩小（只缩不放大，保实物感），不出滚动条——
 * 缩略图要一眼见全貌，裁切和滚动都在撒谎；右上角徽标标注缩放比（100% 不显示）。
 * opts.report = 浮层模式：测量结果 postMessage 给父页（面板据此做 1:1 渲染与尺寸标注），不画徽标。
 */
export function componentPreviewDoc(html: string, opts?: { report?: boolean }): string {
  const body = html
    .replace(/<title>[\s\S]*?<\/title>/g, "")
    .replace(/^\s*<!--[\s\S]*?-->/, "");
  const report = opts?.report
    ? "try{parent.postMessage({mcPreview:{w:root.offsetWidth,h:root.offsetHeight,scale:s}},'*')}catch(e){}"
    : "";
  const badge = opts?.report
    ? ""
    : `<div id="mc-zoom"></div>`;
  return `<!doctype html><html><head><meta charset="utf-8"><style>
html,body{margin:0;padding:0;background:transparent;overflow:hidden}
body{width:100vw;height:100vh;display:grid;place-items:center}
#mc-zoom{position:fixed;top:3px;right:3px;font:600 8px -apple-system,'PingFang SC',sans-serif;color:#cfcfcf;background:rgba(35,35,38,.85);padding:1px 4px;border-radius:4px;display:none}
</style></head><body>${body}${badge}<script>
function mcFit(){
  var root=[].slice.call(document.body.children).find(function(n){return n.id!=='mc-zoom'&&n.tagName!=='STYLE'&&n.tagName!=='SCRIPT'});
  if(!root)return;
  if(getComputedStyle(root).display==='inline')root.style.display='inline-block';
  var s=Math.min(1,(innerWidth-8)/root.offsetWidth,(innerHeight-8)/root.offsetHeight);
  root.style.transformOrigin='center center';
  root.style.transform='scale('+s+')';
  var b=document.getElementById('mc-zoom');
  if(b){if(s<1){b.textContent=Math.round(s*100)+'%';b.style.display='block'}else{b.style.display='none'}}
  ${report}
}
addEventListener('load',mcFit);
addEventListener('resize',mcFit);
if(document.fonts&&document.fonts.ready)document.fonts.ready.then(mcFit);
</script></body></html>`;
}
