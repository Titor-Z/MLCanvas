<script lang="ts">
  import { onMount, tick } from "svelte";
  import { designStore } from "../designStore.svelte.js";
  import { scrollFade } from "../../lib/scrollFade.js";
  import {
    listDesignSystems,
    readDesignSystem,
    type PackIndex,
    type PackSummary,
  } from "../../lib/services/marketService.js";

  /** 列表：vendor 期索引一次读入，store 级缓存（面板开关不重拉） */
  let index = $state<PackIndex | null>(null);
  let loadError = $state("");
  let search = $state("");
  let category = $state("全部");

  /** 详情：选中包 + 该包 tokens.css / 组件页原文 */
  let pack = $state<PackSummary | null>(null);
  let tokens = $state("");
  let componentsHtml = $state("");
  let traits = $state<string[]>([]);
  let detailLoading = $state(false);

  /** 大窗：组件页 100% 原样渲染 */
  let lightbox = $state(false);

  /** 分类中文对照：manifest 原文是英文，未知分类回退英文 */
  const CATEGORY_ZH: Record<string, string> = {
    "AI & LLM": "AI 与大模型",
    "Automotive": "汽车与出行",
    "Backend & Data": "后端与数据",
    "Bold & Expressive": "大胆表达",
    "Creative & Artistic": "创意艺术",
    "Design & Creative": "设计与创作",
    "Developer Tools": "开发者工具",
    "E-Commerce & Retail": "电商零售",
    "Editorial & Print": "编辑与出版",
    "Editorial / Personal / Publication": "编辑·个人·刊物",
    "Editorial · Studio": "编辑·工作室",
    "Fintech & Crypto": "金融科技",
    "Layout & Structure": "版式与结构",
    "Media & Consumer": "媒体与消费",
    "Modern & Minimal": "现代极简",
    "Morphism & Effects": "质感与特效",
    "Productivity & SaaS": "效率与 SaaS",
    "Professional & Corporate": "专业与企业",
    "Retro & Nostalgic": "复古怀旧",
    "Social & Messaging": "社交与通讯",
    "Starter": "入门精选",
    "Themed & Unique": "主题特色",
  };
  const zhCategory = (c: string) => CATEGORY_ZH[c] ?? c;

  const catItems = $derived(
    index
      ? [
          { key: "全部", label: "全部" },
          ...Array.from(new Set(index.packs.map((p) => p.category).filter(Boolean)))
            .sort()
            .map((c) => ({ key: c, label: zhCategory(c) })),
        ]
      : []
  );
  const filtered = $derived(
    index
      ? index.packs.filter((p) => {
          if (category !== "全部" && p.category !== category) return false;
          const q = search.trim().toLowerCase();
          if (!q) return true;
          return (
            p.name.toLowerCase().includes(q) ||
            p.id.includes(q) ||
            p.description.toLowerCase().includes(q)
          );
        })
      : []
  );

  onMount(() => {
    if (index) return;
    listDesignSystems()
      .then((i) => (index = i))
      .catch((e) => (loadError = String(e).replace(/^.*Error:\s*/, "").trim() || "市场数据加载失败"));
  });

  async function openPack(p: PackSummary) {
    pack = p;
    tokens = "";
    componentsHtml = "";
    traits = [];
    detailLoading = true;
    try {
      const [tk, html, design] = await Promise.all([
        readDesignSystem(p.id, "tokens.css").catch(() => null),
        p.hasComponents ? readDesignSystem(p.id, "components.html").catch(() => null) : null,
        readDesignSystem(p.id, "DESIGN.md").catch(() => null),
      ]);
      tokens = tk?.content ?? "";
      componentsHtml = html?.content ?? "";
      traits = extractTraits(design?.content ?? "");
    } finally {
      detailLoading = false;
    }
  }

  /** 签名特征：DESIGN.md「Visual Theme」节的前几条要点；取不到就整节隐藏 */
  function extractTraits(md: string): string[] {
    const lines = md.split("\n");
    const out: string[] = [];
    let inSection = false;
    for (const line of lines) {
      const t = line.trim();
      if (t.startsWith("## ")) {
        if (inSection) break;
        inSection = /visual theme|atmosphere/i.test(t);
        continue;
      }
      if (inSection && t.startsWith("- ") && t.length > 8) {
        out.push(t.slice(2).trim());
        if (out.length >= 3) break;
      }
    }
    return out;
  }

  /** 包令牌 → 详情页调色板（schema 角色名 + 原值） */
  const palette = $derived.by(() => {
    if (!tokens) return [] as { role: string; value: string }[];
    const body = tokens.replace(/\/\*[\s\S]*?\*\//g, "");
    const map: Record<string, string> = {};
    for (const m of body.matchAll(/--([a-z0-9-]+)\s*:\s*([^;]+);/gi)) {
      const v = m[2].trim();
      if (!v.startsWith("var(") && !(m[1] in map)) map[m[1]] = v;
    }
    const roles: [string, string[]][] = [
      ["画布", ["bg"]],
      ["文字", ["fg"]],
      ["表面", ["surface"]],
      ["描边", ["border"]],
      ["弱文字", ["muted"]],
      ["强调", ["accent"]],
      ["成功", ["success"]],
      ["危险", ["danger"]],
    ];
    return roles
      .map(([role, keys]) => {
        const value = keys.map((k) => map[k]).find(Boolean);
        return value ? { role, value } : null;
      })
      .filter((x): x is { role: string; value: string } => x !== null)
      .slice(0, 8);
  });

  /** 字族与字距（tokens.css 的 font/tracking 令牌；取不到隐藏该节） */
  const typeInfo = $derived.by(() => {
    if (!tokens) return null;
    const body = tokens.replace(/\/\*[\s\S]*?\*\//g, "");
    const map: Record<string, string> = {};
    for (const m of body.matchAll(/--([a-z0-9-]+)\s*:\s*([^;]+);/gi)) {
      const v = m[2].trim();
      if (!v.startsWith("var(") && !(m[1] in map)) map[m[1]] = v;
    }
    const family = map["font-display"] ?? map["font-body"] ?? map["font-sans"];
    if (!family) return null;
    const tracking = map["tracking-display"];
    const weight = map["weight-display"] ?? map["weight-heading"];
    return { family, tracking, weight };
  });

  let copied = $state(false);
  async function applyPack() {
    if (!pack) return;
    const phrase =
      `请为当前项目应用「${pack.name}」风格包：用 MCP 的 get_design_system 读取它的 DESIGN.md 与 tokens.css，` +
      `把包的令牌翻译为本项目的命名规范（--color- 等角色制）后用 save_design_spec 写入，并同步更新 design.md。`;
    try {
      await navigator.clipboard.writeText(phrase);
    } catch {
      const ta = document.createElement("textarea");
      ta.value = phrase;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand("copy");
      ta.remove();
    }
    copied = true;
    setTimeout(() => (copied = false), 2000);
  }

  /** 大窗文档：原样 srcdoc + 注入全局滚动条规则（亮面变体，与壳内一致） */
  const lightboxDoc = $derived(
    componentsHtml
      ? componentsHtml.replace(
          /<\/head>/i,
          '<style>::-webkit-scrollbar{width:8px;height:8px}::-webkit-scrollbar-thumb{background:rgba(0,0,0,.2);border-radius:4px}::-webkit-scrollbar-thumb:hover{background:rgba(0,0,0,.34)}::-webkit-scrollbar-track{background:transparent}</style></head>'
        )
      : ""
  );

  async function back() {
    pack = null;
    lightbox = false;
    await tick();
    if (listEl) listEl.scrollTop = listTop;
  }

  function onLightboxKey(e: KeyboardEvent) {
    if (e.key === "Escape") lightbox = false;
  }

  // 分类行拖动横向滚动：按下拖移，越过 5px 视为拖拽并抑制其后的点击
  let chipsEl = $state<HTMLElement | null>(null);
  let chipsDrag = $state<{ x: number; left: number } | null>(null);
  let chipsMoved = $state(false);
  let chipsSuppressClickUntil = 0;

  function onChipsDown(e: PointerEvent) {
    if (e.button !== 0 || !chipsEl) return;
    chipsDrag = { x: e.clientX, left: chipsEl.scrollLeft };
    chipsMoved = false;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }
  function onChipsMove(e: PointerEvent) {
    if (!chipsDrag || !chipsEl) return;
    const dx = e.clientX - chipsDrag.x;
    if (Math.abs(dx) > 5) chipsMoved = true;
    chipsEl.scrollLeft = chipsDrag.left - dx;
  }
  function onChipsUp() {
    if (chipsMoved) chipsSuppressClickUntil = Date.now() + 300;
    chipsDrag = null;
  }
  function onChipClick(c: string) {
    if (Date.now() < chipsSuppressClickUntil) return;
    category = c;
  }

  // 返回列表恢复滚动位置：列表在详情期间被卸载，回来后滚回原位
  let listEl = $state<HTMLElement | null>(null);
  let listTop = 0;
</script>

<aside class="market">
  {#if !pack}
    <!-- 列表 -->
    <div class="market__head">
      <div class="market__title">市场</div>
      <div class="market__sub">风格参考库 · 应用由 Agent 翻译为项目规范</div>
    </div>
    <div class="market__search">
      <svg viewBox="0 0 12 12"><circle cx="5" cy="5" r="3.4" /><path d="M7.6 7.6 10.4 10.4" /></svg>
      <input placeholder="搜索风格包" bind:value={search} />
    </div>
    {#if catItems.length > 1}
      <div
        class="chips"
        bind:this={chipsEl}
        onpointerdown={onChipsDown}
        onpointermove={onChipsMove}
        onpointerup={onChipsUp}
        onpointercancel={onChipsUp}
      >
        {#each catItems as c (c.key)}
          <button
            class="chip"
            class:chip--active={category === c.key}
            onclick={() => onChipClick(c.key)}
          >{c.label}</button>
        {/each}
      </div>
    {/if}

    {#if loadError}
      <div class="market__empty">{loadError}</div>
    {:else if !index}
      <!-- 骨架屏：索引读取期间的占位（尺寸=卡片尺寸，数据到达不跳版） -->
      <div class="market__scroll" use:scrollFade>
        {#each Array(5) as _, i (i)}
          <div class="skel">
            <div class="skel__cover"></div>
            <div class="skel__name"></div>
            <div class="skel__desc"></div>
          </div>
        {/each}
      </div>
    {:else}
      <div class="market__scroll" use:scrollFade bind:this={listEl} onscroll={(e) => (listTop = e.currentTarget.scrollTop)}>
        {#each filtered as p (p.id)}
          <div
            class="pack"
            role="button"
            tabindex="0"
            onclick={() => openPack(p)}
            onkeydown={(e) => e.key === "Enter" && openPack(p)}
          >
            <div
              class="cover"
              style="--c-bg:{p.cover.bg}; --c-fg:{p.cover.fg}; --c-mut:{p.cover.mut}; --c-bd:{p.cover.bd}; --c-btn:{p.cover.btn}; --c-ac:{p.cover.btn}; --c-r:{p.cover.r}"
            >
              <div class="cover__nav"><i class="cover__dot"></i><i class="cover__ln"></i><i class="cover__ln"></i><i class="cover__cta"></i></div>
              <div class="cover__hl"></div>
              <div class="cover__sub"></div>
              <div class="cover__actions"><i class="cover__btn"></i><i class="cover__ghost"></i></div>
            </div>
            <div class="pack__row">
              <span class="pack__name">{p.name}</span>
              {#if p.category}<span class="pack__cat">{zhCategory(p.category)}</span>{/if}
            </div>
            {#if p.description && !p.description.includes("Bundled OpenDesign package")}
              <div class="pack__desc">{p.description}</div>
            {/if}
          </div>
        {:else}
          <div class="market__empty">没有匹配的风格包</div>
        {/each}
      </div>
    {/if}
  {:else}
    <!-- 详情 -->
    <div class="market__top">
      <button class="market__back" title="返回市场" onclick={back}>
        <svg viewBox="0 0 12 12"><path d="M7.5 2 3.5 6l4 4" /></svg>
      </button>
      <span class="market__crumb">市场</span>
    </div>
    <div class="market__scroll market__scroll--detail" use:scrollFade>
      <div class="det__name">{pack.name}</div>
      {#if pack.category}<div class="det__cat">{zhCategory(pack.category)}</div>{/if}
      {#if pack.description && !pack.description.includes("Bundled OpenDesign package")}
        <div class="det__desc">{pack.description}</div>
      {/if}

      {#if detailLoading}
        <div class="det__section"><div class="skel__cover" style="height:110px"></div></div>
      {:else if componentsHtml}
        <div class="det__section">
          <div class="det__label">组件 · 包内原样</div>
          <div
            class="pageview"
            role="button"
            tabindex="0"
            title="放大查看"
            onclick={() => (lightbox = true)}
            onkeydown={(e) => e.key === "Enter" && (lightbox = true)}
          >
            <iframe title="{pack.name} 组件页预览" srcdoc={componentsHtml} scrolling="no"></iframe>
            <div class="badge-scale">预览</div>
          </div>
        </div>
      {/if}

      {#if palette.length}
        <div class="det__section">
          <div class="det__label">色彩 · {palette.length} 个角色</div>
          <div class="palette">
            {#each palette as c (c.role)}
              <div class="pal">
                <div class="pal__sw" style="background:{c.value}"></div>
                <div class="pal__meta">
                  <div class="pal__role">{c.role}</div>
                  <div class="pal__hex">{c.value}</div>
                </div>
              </div>
            {/each}
          </div>
        </div>
      {/if}

      {#if typeInfo}
        <div class="det__section">
          <div class="det__label">字体</div>
          <div class="type">
            <div class="type__ag">Ag</div>
            <div class="type__meta">
              <div class="type__family">{typeInfo.family}</div>
              <div class="type__rows">
                {#if typeInfo.weight}<div class="type__row">标题字重 {typeInfo.weight}</div>{/if}
                {#if typeInfo.tracking}<div class="type__row">显示字距 {typeInfo.tracking}</div>{/if}
              </div>
            </div>
          </div>
        </div>
      {/if}

      {#if traits.length}
        <div class="det__section">
          <div class="det__label">签名特征</div>
          <div class="traits">
            {#each traits as t (t)}
              <div class="trait"><i></i>{t}</div>
            {/each}
          </div>
        </div>
      {/if}
    </div>

    <div class="market__cta">
      <button class="market__apply" onclick={applyPack}>{copied ? "已复制指令 · 粘贴给 Agent" : "应用到项目"}</button>
      <div class="market__note">Agent 读取此包并翻译为项目的令牌与规范</div>
    </div>
  {/if}
</aside>

{#if lightbox && pack}
  <!-- 组件大窗：浮在画布上，面板保持可见；Esc / 点外部收起 -->
  <div class="lb-scrim" role="presentation" onclick={() => (lightbox = false)}>
    <div class="lightbox" role="dialog" onclick={(e) => e.stopPropagation()}>
      <div class="lightbox__head">
        <span class="lightbox__title">{pack.name}</span>
        <span class="lightbox__tag">组件 · 包内原样</span>
        <span class="lightbox__badge">100%</span>
        <button class="lightbox__close" title="关闭" onclick={() => (lightbox = false)}>
          <svg viewBox="0 0 12 12"><path d="M2 2l8 8M10 2l-8 8" /></svg>
        </button>
      </div>
      <div class="lightbox__body" use:scrollFade>
        <iframe title="{pack.name} 组件页" srcdoc={lightboxDoc}></iframe>
      </div>
    </div>
  </div>
{/if}

<svelte:window onkeydown={(e) => lightbox && onLightboxKey(e)} />

<style>
  .market {
    height: 100%;
    min-height: 0;
    position: relative;
    background: var(--chrome);
    border-right: 1px solid var(--line);
    display: flex; flex-direction: column;
    overflow: hidden;
  }
  .market__head { padding: 13px 14px 0; }
  .market__title { font-size: 13px; font-weight: 600; }
  .market__sub { margin-top: 3px; font-size: 11px; color: var(--text-3); line-height: 1.5; }
  .market__search {
    margin: 10px 14px 0;
    height: 28px; padding: 0 9px;
    background: var(--field); border-radius: 5px;
    display: flex; align-items: center; gap: 6px;
    color: var(--text-3);
  }
  .market__search svg { width: 11px; height: 11px; fill: none; stroke: currentColor; stroke-width: 1.4; stroke-linecap: round; flex-shrink: 0; }
  .market__search input {
    flex: 1; min-width: 0; height: 100%;
    background: none; border: none; outline: none;
    font-size: 12px; color: var(--text);
  }
  .chips { display: flex; gap: 5px; padding: 10px 14px 2px; overflow-x: auto; }
  .chips::-webkit-scrollbar { display: none; }
  .chip {
    flex-shrink: 0; padding: 3px 8px; border-radius: 20px;
    font-size: 11px; color: var(--text-2);
    background: var(--field);
  }
  .chip--active { background: var(--accent-soft); color: var(--accent); font-weight: 600; }
  .market__scroll { flex: 1; min-height: 0; overflow-y: auto; padding: 10px 14px 14px; display: grid; grid-template-columns: minmax(0, 1fr); gap: 12px; align-content: start; }
  .market__scroll--detail { padding: 16px 16px 122px; }
  .market__empty { padding: 10px 4px; font-size: 11.5px; color: var(--text-3); line-height: 1.6; }

  /* 封面卡 */
  .pack { border-radius: 8px; cursor: default; }
  .pack:active { transform: scale(0.98); }
  .cover {
    height: 96px;
    border-radius: 7px;
    background: var(--c-bg);
    border: 1px solid rgba(255, 255, 255, 0.1);
    padding: 10px 11px;
    overflow: hidden;
    margin-bottom: 8px;
  }
  .cover__nav { display: flex; align-items: center; gap: 4px; }
  .cover__dot { width: 8px; height: 8px; border-radius: 2.5px; background: var(--c-ac); flex-shrink: 0; }
  .cover__ln { width: 20px; height: 4px; border-radius: 2px; background: var(--c-bd); }
  .cover__cta { margin-left: auto; width: 24px; height: 10px; border-radius: var(--c-r); background: var(--c-btn); }
  .cover__hl { margin-top: 12px; width: 64%; height: 8px; border-radius: 3px; background: var(--c-fg); opacity: 0.92; }
  .cover__sub { margin-top: 6px; width: 46%; height: 5px; border-radius: 2.5px; background: var(--c-mut); opacity: 0.75; }
  .cover__actions { display: flex; gap: 5px; margin-top: 12px; }
  .cover__btn { width: 44px; height: 17px; border-radius: var(--c-r); background: var(--c-btn); }
  .cover__ghost { width: 34px; height: 17px; border-radius: var(--c-r); border: 1px solid var(--c-bd); }
  .pack__row { display: flex; align-items: center; gap: 5px; padding: 0 2px; }
  .pack__name { font-size: 12px; font-weight: 600; }
  .pack__cat { margin-left: auto; font-size: 10px; color: var(--text-3); }
  .pack__desc {
    padding: 2px 2px 0;
    font-size: 10.5px; color: var(--text-2);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
    min-width: 0;
  }

  /* 骨架屏 */
  .skel { border-radius: 8px; }
  .skel__cover {
    height: 96px; border-radius: 7px;
    background: var(--field); margin-bottom: 8px;
    animation: skel-pulse 1.6s ease-in-out infinite;
  }
  .skel__name { height: 11px; width: 40%; border-radius: 3px; background: var(--field); margin: 0 2px 6px; animation: skel-pulse 1.6s ease-in-out infinite; }
  .skel__desc { height: 9px; width: 72%; border-radius: 3px; background: var(--field); margin: 0 2px; animation: skel-pulse 1.6s ease-in-out infinite; }
  @keyframes skel-pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.55; } }

  /* 详情 */
  .market__top {
    display: flex; align-items: center; gap: 6px;
    padding: 10px 12px 9px;
    border-bottom: 1px solid var(--line);
    flex-shrink: 0;
  }
  .market__back {
    width: 22px; height: 22px;
    display: grid; place-items: center;
    border-radius: 5px; color: var(--text-2);
  }
  .market__back:hover { background: var(--field); color: var(--text); }
  .market__back svg { width: 12px; height: 12px; fill: none; stroke: currentColor; stroke-width: 1.6; stroke-linecap: round; stroke-linejoin: round; }
  .market__crumb { font-size: 11px; color: var(--text-3); }
  .det__name { font-size: 16px; font-weight: 700; letter-spacing: -0.3px; }
  .det__cat { margin-top: 3px; font-size: 10.5px; color: var(--text-3); }
  .det__desc { margin-top: 10px; font-size: 11px; color: var(--text-2); line-height: 1.6; }
  .det__section { margin-top: 22px; }
  .det__label { font-size: 10px; color: var(--text-3); letter-spacing: 1px; margin-bottom: 10px; }

  .pageview {
    position: relative;
    height: 118px;
    border-radius: 8px; overflow: hidden;
    border: 1px solid rgba(255, 255, 255, 0.09);
    background: #fff;
    cursor: zoom-in;
  }
  /* iframe 绝对定位：640px 布局宽不撑开滚动区（transform 不影响布局尺寸的老坑） */
  .pageview iframe { position: absolute; top: 0; left: 0; width: 285.714%; height: 340px; border: 0; transform: scale(0.35); transform-origin: 0 0; pointer-events: none; background: #fff; }
  .badge-scale {
    position: absolute; top: 8px; right: 8px;
    height: 16px; padding: 0 6px;
    background: rgba(30, 30, 30, 0.72); border-radius: 4px;
    font-size: 9px; color: #e8e8e8;
    display: flex; align-items: center;
    font-family: "SF Mono", Menlo, monospace;
  }

  .palette { display: grid; grid-template-columns: repeat(2, 1fr); gap: 9px 8px; }
  .pal { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .pal__sw { width: 32px; height: 32px; flex-shrink: 0; border-radius: 6px; border: 1px solid rgba(255, 255, 255, 0.1); }
  .pal__meta { min-width: 0; }
  .pal__role { font-size: 10.5px; }
  .pal__hex { margin-top: 1px; font-size: 9.5px; color: var(--text-3); font-family: "SF Mono", Menlo, monospace; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  .type { display: flex; align-items: center; gap: 14px; }
  .type__ag { font-size: 46px; font-weight: 700; letter-spacing: -1.5px; line-height: 1; }
  .type__meta { min-width: 0; }
  .type__family { font-size: 11.5px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .type__rows { margin-top: 6px; display: grid; gap: 4px; }
  .type__row { font-size: 9.5px; color: var(--text-3); }

  .traits { display: grid; gap: 8px; }
  .trait { display: flex; gap: 8px; font-size: 10.5px; color: var(--text-2); line-height: 1.55; }
  .trait i { width: 4px; height: 4px; flex-shrink: 0; margin-top: 6px; border-radius: 50%; background: var(--text-3); }

  /* CTA */
  .market__cta {
    position: absolute; left: 0; right: 0; bottom: 0;
    padding: 24px 14px 12px;
    background: linear-gradient(to top, var(--chrome) 82%, rgba(30, 30, 30, 0));
  }
  .market__apply {
    width: 100%; height: 32px; border-radius: 6px;
    background: var(--accent);
    font-size: 12px; font-weight: 600; color: #fff;
  }
  .market__apply:active { transform: scale(0.98); }
  .market__note { margin-top: 6px; text-align: center; font-size: 10px; color: var(--text-3); }

  /* 组件大窗 */
  .lb-scrim { position: fixed; inset: 0; z-index: 80; }
  .lightbox {
    position: absolute;
    top: 44px; bottom: 0;
    left: calc(44px + 256px); right: 264px;
    background: #26262a;
    border-radius: 10px;
    box-shadow: 0 18px 60px rgba(0, 0, 0, 0.6);
    display: flex; flex-direction: column;
    overflow: hidden;
  }
  .lightbox__head {
    display: flex; align-items: center; gap: 8px;
    height: 40px; padding: 0 14px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    flex-shrink: 0;
  }
  .lightbox__title { font-size: 12px; font-weight: 600; }
  .lightbox__tag { font-size: 10px; color: var(--text-3); }
  .lightbox__badge {
    height: 17px; padding: 0 6px;
    background: rgba(255, 255, 255, 0.1); border-radius: 4px;
    font-size: 9.5px; color: var(--text-2);
    font-family: "SF Mono", Menlo, monospace;
    display: flex; align-items: center;
  }
  .lightbox__close {
    margin-left: auto;
    width: 24px; height: 24px;
    display: grid; place-items: center;
    border-radius: 5px; color: var(--text-2);
  }
  .lightbox__close:hover { background: rgba(255, 255, 255, 0.1); color: var(--text); }
  .lightbox__close svg { width: 11px; height: 11px; fill: none; stroke: currentColor; stroke-width: 1.4; stroke-linecap: round; }
  .lightbox__body { position: relative; flex: 1; overflow-y: auto; background: #fff; }
  .lightbox__body :global(.sb-fade) { position: relative; }
  .lightbox__body iframe { width: 100%; height: 100%; border: 0; display: block; }
</style>
