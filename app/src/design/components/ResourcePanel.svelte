<script lang="ts">
  import { scrollFade } from "../../lib/scrollFade.js";
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import {
    specService,
    parseSpec,
    componentPreviewDoc,
    type SpecView,
    type ComponentInfo,
  } from "../../lib/services/specService.js";

  let loading = $state(true);
  let hasSpec = $state(false);
  let spec = $state<SpecView | null>(null);
  let components = $state<ComponentInfo[]>([]);
  let search = $state("");
  /** 分区折叠态（默认折「间距/圆角投影」，低频项） */
  let collapsed = $state<Record<string, boolean>>({ spacing: true, shapes: true });

  async function load() {
    try {
      const raw = await specService.get();
      hasSpec = !!(raw.design_md.trim() || raw.tokens_css.trim());
      spec = hasSpec ? parseSpec(raw) : null;
      components = await specService.components();
    } catch {
      hasSpec = false;
      spec = null;
      components = [];
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    load();
    const stop = listen("resources-changed", () => load());
    window.addEventListener("message", onPreviewMessage);
    return () => {
      stop.then((off) => off());
      window.removeEventListener("message", onPreviewMessage);
      clearTimeout(hoverTimer);
    };
  });

  const q = $derived(search.trim().toLowerCase());
  const hit = (text: string) => !q || text.toLowerCase().includes(q);
  const colors = $derived(
    (spec?.colors ?? []).filter((c) => hit(c.name) || hit(c.label ?? "") || hit(c.value)),
  );
  const types = $derived(
    (spec?.types ?? []).filter((t) => hit(t.name) || hit(t.label ?? "") || hit(String(t.size))),
  );
  const spaces = $derived(
    (spec?.spaces ?? []).filter((s) => hit(s.name) || hit(s.label ?? "") || hit(String(s.value))),
  );
  const radii = $derived(
    (spec?.radii ?? []).filter((r) => hit(r.name) || hit(r.label ?? "") || hit(String(r.value))),
  );
  const shadows = $derived(
    (spec?.shadows ?? []).filter((s) => hit(s.name) || hit(s.label ?? "") || hit(s.value)),
  );
  const bans = $derived((spec?.bans ?? []).filter((b) => hit(b)));
  const comps = $derived(components.filter((c) => hit(c.name) || hit(c.source)));

  /** 搜索时强制展开全部，避免"搜到了却是折着的" */
  function isOpen(key: string): boolean {
    return !!q || !collapsed[key];
  }
  function toggle(key: string): void {
    collapsed[key] = !collapsed[key];
  }

  /** 风格身份块：主色圆牌 + 内点（规范自己的颜色） */
  const markBg = $derived(spec?.colors[0]?.value ?? "#111111");
  const markDot = $derived(spec?.colors[1]?.value ?? "#f4f4f1");

  /** 悬停浮层：1:1 实物预览（暗色、无内框、origin 动画——设计稿 ui-component-hover.html 定稿） */
  let preview = $state<
    | null
    | {
        key: string;
        name: string;
        src: string;
        doc: string;
        /** 视口定位：above 用 bottom 锚卡片上方，below 用 top */
        left: number;
        bottom: number | null;
        top: number | null;
        arrowX: number;
        below: boolean;
        fw: number;
        fh: number;
        badge: string;
        dims: string;
      }
  >(null);
  let hoverTimer: ReturnType<typeof setTimeout> | undefined;

  function hidePreview(): void {
    clearTimeout(hoverTimer);
    hoverTimer = undefined;
    preview = null;
  }

  function schedulePreview(c: ComponentInfo, tile: HTMLElement): void {
    clearTimeout(hoverTimer);
    // 连续扫过卡片时立即切换（Emil 的 tooltip 惯例），首次悬停留 150ms 防误触
    if (preview) {
      openPreview(c, tile);
      return;
    }
    hoverTimer = setTimeout(() => openPreview(c, tile), 150);
  }

  function openPreview(c: ComponentInfo, tile: HTMLElement): void {
    const rect = tile.getBoundingClientRect();
    const arrowX = Math.max(20, Math.round(rect.left + rect.width / 2));
    const below = rect.top < 220; // 上方放不下（浮层估高 ~200px）就翻到卡片下方
    preview = {
      key: c.file_name,
      name: c.name,
      src: c.source,
      doc: componentPreviewDoc(c.html, { report: true }),
      left: Math.max(10, Math.min(window.innerWidth - 280, arrowX - 130)),
      bottom: below ? null : window.innerHeight - rect.top + 10,
      top: below ? rect.bottom + 10 : null,
      arrowX,
      below,
      fw: 120,
      fh: 60,
      badge: "1 : 1",
      dims: "",
    };
  }

  /** 浮层 iframe 上报组件实际尺寸 → 面板按 1:1（超限等比缩）定 iframe 尺寸与徽标 */
  function onPreviewMessage(e: MessageEvent): void {
    const d = (e.data as { mcPreview?: { w: number; h: number; scale: number } } | null)?.mcPreview;
    if (!d || !preview) return;
    const s = Math.min(1, 340 / d.w, 280 / d.h);
    preview = {
      ...preview,
      fw: Math.max(60, Math.round(d.w * s) + 16),
      fh: Math.max(44, Math.round(d.h * s) + 12),
      badge: s >= 0.995 ? "1 : 1" : `${Math.round(s * 100)}%`,
      dims: `${Math.round(d.w)} × ${Math.round(d.h)}`,
    };
  }
  const emptyResult = $derived(
    !!q &&
      colors.length === 0 &&
      types.length === 0 &&
      spaces.length === 0 &&
      radii.length === 0 &&
      shadows.length === 0 &&
      comps.length === 0 &&
      bans.length === 0,
  );
</script>

{#if loading}
  <div class="res__loading">正在读取资源…</div>
{:else if !hasSpec}
  <!-- 空态：路标是「去 Agent 里说」 -->
  <div class="res__empty">
    <div class="res__empty-icon"><svg viewBox="0 0 16 16"><rect x="2" y="2" width="5.2" height="5.2" rx="1.4"/><rect x="8.8" y="2" width="5.2" height="5.2" rx="1.4"/><rect x="2" y="8.8" width="5.2" height="5.2" rx="1.4"/><rect x="8.8" y="8.8" width="5.2" height="5.2" rx="1.4"/></svg></div>
    <div class="res__empty-title">还没有风格规范</div>
    <div class="res__empty-body">
      在 Agent 里描述你想要的风格，比如<b>「我要一个安静、留白多的极简风格」</b>，Agent 会把规范定在这里，之后的画板都会遵守它。
    </div>
    <div class="res__empty-alt">也可以让 Agent 参考画布上已有的画板来提炼。</div>
  </div>
{:else if spec}
  <div class="res__head">
    <div class="res__mark" style="background:{markBg}"><i style="background:{markDot}"></i></div>
    <div class="res__id">
      <div class="res__name">{spec.name || "未命名风格"}</div>
      <div class="res__meta"><i></i>Agent 依对话维护</div>
    </div>
  </div>

  <div class="res__search">
    <input placeholder="搜索资源" bind:value={search} />
  </div>

  <div class="res__scroll" onscroll={hidePreview}>
    {#if colors.length}
      <div class="sec" role="button" tabindex="0" onclick={() => toggle("colors")} onkeydown={(e) => e.key === "Enter" && toggle("colors")}>
        <span class="sec__chev" class:closed={!isOpen("colors")}><svg viewBox="0 0 8 8"><path d="M1.5 2.75 L4 5.25 L6.5 2.75"/></svg></span>
        <span>色彩</span><span class="sec__count">{colors.length}</span>
      </div>
      {#if isOpen("colors")}
        <div class="swatches">
          {#each colors as c (c.name)}
            <div title="{c.name} · {c.value}">
              <div class="sw__chip" style="background:{c.value}"></div>
              <div class="sw__name">{c.label ?? c.name}</div>
              <div class="sw__val"><span class="sw__hex">{c.value.replace(/^#/, "")}</span><span class="sw__token">{c.name}</span></div>
            </div>
          {/each}
        </div>
      {/if}
    {/if}

    {#if types.length}
      <div class="sec" role="button" tabindex="0" onclick={() => toggle("types")} onkeydown={(e) => e.key === "Enter" && toggle("types")}>
        <span class="sec__chev" class:closed={!isOpen("types")}><svg viewBox="0 0 8 8"><path d="M1.5 2.75 L4 5.25 L6.5 2.75"/></svg></span>
        <span>字阶</span><span class="sec__count">{types.length}</span>
      </div>
      {#if isOpen("types")}
        <div class="type">
          {#each types as t (t.name)}
            <div class="type__row" title="{t.name} · {t.size}px">
              <span class="type__demo" style="font-size:{Math.min(t.size, 24)}px;font-weight:{t.weight}">{t.label ?? t.name}</span>
              <span class="type__label">{t.size}</span>
            </div>
          {/each}
        </div>
      {/if}
    {/if}

    {#if spaces.length}
      <div class="sec" role="button" tabindex="0" onclick={() => toggle("spacing")} onkeydown={(e) => e.key === "Enter" && toggle("spacing")}>
        <span class="sec__chev" class:closed={!isOpen("spacing")}><svg viewBox="0 0 8 8"><path d="M1.5 2.75 L4 5.25 L6.5 2.75"/></svg></span>
        <span>间距</span><span class="sec__count">{spaces.length}</span>
      </div>
      {#if isOpen("spacing")}
        <div class="spaces">
          {#each spaces as s (s.name)}
            <div class="space__row" title="{s.name} · {s.value}px">
              <span class="space__bar" style="width:{Math.min(s.value, 64)}px"></span>
              <span class="space__label">{s.value}</span>
              <span class="space__name">{s.label ?? s.name}</span>
            </div>
          {/each}
        </div>
      {/if}
    {/if}

    {#if radii.length || shadows.length}
      <div class="sec" role="button" tabindex="0" onclick={() => toggle("shapes")} onkeydown={(e) => e.key === "Enter" && toggle("shapes")}>
        <span class="sec__chev" class:closed={!isOpen("shapes")}><svg viewBox="0 0 8 8"><path d="M1.5 2.75 L4 5.25 L6.5 2.75"/></svg></span>
        <span>圆角 · 投影</span><span class="sec__count">{radii.length + shadows.length}</span>
      </div>
      {#if isOpen("shapes")}
        {#if radii.length}
          <div class="shapes">
            {#each radii as r (r.name)}
              <div class="shape" style="border-radius:{r.value}px" title="{r.label ?? r.name} · {r.value}px">{r.value}</div>
            {/each}
          </div>
        {/if}
        {#if shadows.length}
          <div class="shadows">
            {#each shadows as s (s.name)}
              <div class="shadow__item" title="{s.name} · {s.value}">
                <i class="shadow__card" style="box-shadow:{s.value}"></i>
                <span class="shadow__cn">{s.label ?? s.name}</span>
                <span class="shadow__token">{s.name}</span>
              </div>
            {/each}
          </div>
        {/if}
      {/if}
    {/if}

    {#if comps.length}
      <div class="sec" role="button" tabindex="0" onclick={() => toggle("components")} onkeydown={(e) => e.key === "Enter" && toggle("components")}>
        <span class="sec__chev" class:closed={!isOpen("components")}><svg viewBox="0 0 8 8"><path d="M1.5 2.75 L4 5.25 L6.5 2.75"/></svg></span>
        <span>组件</span><span class="sec__count">{comps.length}</span>
      </div>
      {#if isOpen("components")}
        <div class="cgrid">
          {#each comps as c (c.file_name)}
            <div class="ccard">
              <div
                class="ccard__tile"
                onmouseenter={(e) => schedulePreview(c, e.currentTarget as HTMLElement)}
                onmouseleave={hidePreview}
              ><iframe class="ccard__frame" title={c.name} srcdoc={componentPreviewDoc(c.html)}></iframe></div>
              <div class="ccard__name">{c.name}</div>
              {#if c.source}<div class="ccard__src">来自 {c.source}</div>{/if}
            </div>
          {/each}
        </div>
      {/if}
    {/if}

    {#if bans.length}
      <div class="sec" role="button" tabindex="0" onclick={() => toggle("bans")} onkeydown={(e) => e.key === "Enter" && toggle("bans")}>
        <span class="sec__chev" class:closed={!isOpen("bans")}><svg viewBox="0 0 8 8"><path d="M1.5 2.75 L4 5.25 L6.5 2.75"/></svg></span>
        <span>设计禁令</span><span class="sec__count">{bans.length}</span>
      </div>
      {#if isOpen("bans")}
        <div class="bans">
          {#each bans as b (b)}
            <div class="ban"><i>✕</i>{b}</div>
          {/each}
        </div>
      {/if}
    {/if}

    {#if emptyResult}
      <div class="res__noresult">没有匹配的资源。</div>
    {/if}
  </div>

  {#if preview}
    <div
      class="cpop"
      class:cpop--below={preview.below}
      style="left:{preview.left}px;{preview.below
        ? `top:${preview.top}px`
        : `bottom:${preview.bottom}px`}"
      onmouseenter={() => clearTimeout(hoverTimer)}
      onmouseleave={hidePreview}
    >
      <span class="cpop__arrow" style="left:{Math.max(14, Math.min(232, preview.arrowX - preview.left))}px"></span>
      <span class="cpop__badge">{preview.badge}</span>
      <iframe class="cpop__frame" title={preview.name} srcdoc={preview.doc} style="width:{preview.fw}px;height:{preview.fh}px"></iframe>
      <div class="cpop__meta">
        <span class="cpop__cn">{preview.name}</span>
        <span class="cpop__src">来自 {preview.src}</span>
        <span class="cpop__dim">{preview.dims}</span>
      </div>
    </div>
  {/if}
{/if}

<style>
  .res__loading,
  .res__noresult {
    padding: 12px 14px;
    font-size: 12px;
    color: var(--text-3);
  }
  /* 空态：居中路标（wayfinding），引导去 Agent */
  .res__empty {
    height: 100%;
    display: flex; flex-direction: column;
    align-items: center; justify-content: center;
    padding: 0 26px;
    text-align: center;
  }
  .res__empty-icon {
    width: 44px; height: 44px;
    border-radius: 12px;
    border: 1px dashed rgba(255, 255, 255, 0.22);
    display: grid; place-items: center;
    color: var(--text-3);
    margin-bottom: 14px;
  }
  .res__empty-icon svg { width: 18px; height: 18px; fill: none; stroke: currentColor; stroke-width: 1.3; }
  .res__empty-title { font-size: 13px; font-weight: 600; color: var(--text-2); }
  .res__empty-body { margin-top: 8px; font-size: 11px; line-height: 1.65; color: var(--text-3); }
  .res__empty-body b { color: var(--text-2); font-weight: 600; }
  .res__empty-alt {
    margin-top: 14px; padding-top: 12px;
    border-top: 1px solid var(--line);
    font-size: 11px; line-height: 1.6; color: var(--text-3);
  }

  /* 身份块：这套风格是谁、谁在维护 */
  .res__head {
    padding: 11px 14px 10px;
    border-bottom: 1px solid var(--line);
    display: flex; align-items: center; gap: 10px;
    flex-shrink: 0;
  }
  .res__mark {
    width: 30px; height: 30px; flex-shrink: 0;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.14);
    display: grid; place-items: center;
  }
  .res__mark i { width: 10px; height: 10px; border-radius: 50%; }
  .res__id { min-width: 0; }
  .res__name { font-size: 13px; font-weight: 700; color: var(--text); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .res__meta { margin-top: 2px; font-size: 10px; color: var(--text-3); display: flex; align-items: center; gap: 4px; }
  .res__meta i { width: 5px; height: 5px; border-radius: 50%; background: #28c840; }

  .res__search { padding: 10px 12px; flex-shrink: 0; }
  .res__search input { width: 100%; height: 28px; padding: 0 9px; font-size: 12px; }

  .res__scroll { flex: 1; overflow-y: auto; padding-bottom: 14px; }

  /* 分区头：整行可点，行首 chevron 表达折叠态（disclosure），行尾计数收尾。
     分离靠留白与内容形态差异，不用分隔线——线是同质行之间才用的最后手段 */
  .sec {
    display: flex; align-items: center; gap: 5px;
    padding: 16px 14px 8px;
    font-size: 10px; letter-spacing: 1px;
    color: var(--text-3);
    cursor: default;
    user-select: none;
  }
  .res__scroll .sec:first-child { padding-top: 2px; }
  .sec:hover { color: var(--text-2); }
  .sec__chev { color: var(--text-2); width: 12px; display: grid; place-items: center; flex-shrink: 0; }
  .sec__chev svg { width: 8px; height: 8px; fill: none; stroke: currentColor; stroke-width: 1.2; stroke-linecap: round; stroke-linejoin: round; display: block; transition: transform 160ms ease-out; }
  .sec__chev.closed svg { transform: rotate(-90deg); }
  .sec__count { margin-left: auto; letter-spacing: 0; font-family: "SF Mono", Menlo, monospace; font-size: 9px; }

  /* 色彩：真实色板（两列——窄轨里三列装不下名称+值，密度换呼吸感不值；
     网格项 min-width:0——nowrap 的行内省略不能反向撑宽 1fr 列） */
  .swatches { display: grid; grid-template-columns: repeat(2, 1fr); gap: 10px 8px; padding: 0 14px; }
  .swatches > div { min-width: 0; }
  .sw__chip { height: 30px; border-radius: 7px; border: 1px solid rgba(255, 255, 255, 0.1); margin-bottom: 5px; }
  .sw__name { font-size: 11px; color: var(--text-2); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sw__val { display: flex; align-items: baseline; gap: 4px; font-size: 9px; color: var(--text-3); font-family: "SF Mono", Menlo, monospace; margin-top: 1px; }
  /* hex 在左永不截断，英文名靠右、溢出只吃英文尾巴 */
  .sw__hex { flex-shrink: 0; }
  .sw__token { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; margin-left: auto; }

  /* 字阶：真实字号排出来 */
  .type { padding: 0 14px; display: grid; gap: 8px; }
  .type__row { display: flex; align-items: baseline; justify-content: space-between; gap: 8px; }
  .type__demo { color: var(--text); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; min-width: 0; }
  .type__label { font-size: 10px; color: var(--text-3); flex-shrink: 0; font-family: "SF Mono", Menlo, monospace; }

  /* 间距：真实宽度的标尺条 */
  .spaces { padding: 0 14px; display: grid; gap: 6px; }
  .space__row { display: flex; align-items: center; gap: 8px; }
  .space__bar { height: 8px; border-radius: 2px; background: var(--accent-soft); border: 1px solid rgba(75, 158, 255, 0.35); flex-shrink: 0; }
  .space__label { font-size: 10px; color: var(--text-3); font-family: "SF Mono", Menlo, monospace; flex-shrink: 0; }
  .space__name { margin-left: auto; font-size: 10px; color: var(--text-3); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; min-width: 0; }

  /* 圆角 · 投影：一个分区（对齐 ui-spec.html 原稿）——
     圆角是 34px 实形芯片、值内嵌；投影用实物展台（芯片尺度读不出微阴影，故另用展台） */
  .shapes { padding: 0 14px; display: flex; gap: 10px; flex-wrap: wrap; }
  .shape {
    width: 34px; height: 34px;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.22);
    display: grid; place-items: center;
    font-size: 9px; color: var(--text-3); font-family: "SF Mono", Menlo, monospace;
  }
  .shapes + .shadows { margin-top: 12px; }

  /* 投影：共享浅色样张（一收敛的米白画布承载所有阴影卡，名字标在卡下）——
     替代逐行展台：重复的大井是噪声、打断数据行节奏；样张上多张卡天然可比 */
  .shadows {
    margin: 0 14px;
    background: #f4f4f1;
    border-radius: 8px;
    padding: 12px;
    display: flex; gap: 16px; flex-wrap: wrap;
  }
  .shadow__item { display: grid; gap: 5px; justify-items: start; }
  .shadow__card { width: 80px; height: 40px; background: #ffffff; border-radius: 8px; }
  .shadow__cn { font-size: 10px; color: #3a3a3a; }
  .shadow__token { font-size: 8px; color: #8a8a8a; font-family: "SF Mono", Menlo, monospace; }

  /* 组件：实物预览（iframe 原尺寸） + 来源 */
  .cgrid { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; padding: 0 14px; }
  .ccard { min-width: 0; }
  .ccard__tile {
    height: 62px;
    border-radius: 7px;
    background: #f4f4f1;
    overflow: hidden;
    margin-bottom: 6px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    display: flex; align-items: center; justify-content: center;
  }
  .ccard__frame { width: 100%; height: 100%; border: 0; display: block; }
  .ccard__name { font-size: 11px; color: var(--text-2); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .ccard__src { font-size: 9px; color: var(--text-3); margin-top: 1px; }

  /* 设计禁令 */
  .bans { padding: 0 14px; display: grid; gap: 6px; }
  .ban { display: flex; align-items: flex-start; gap: 7px; font-size: 11px; color: var(--text-2); line-height: 1.45; }
  .ban i {
    width: 13px; height: 13px; flex-shrink: 0; margin-top: 1px;
    border-radius: 50%;
    background: rgba(229, 72, 77, 0.18);
    color: #e5484d;
    font-size: 8px; font-style: normal;
    display: grid; place-items: center;
  }

  /* 悬停浮层：暗色、无内框、1:1 实物 + 尺寸标注（设计稿 ui-component-hover.html 定稿）。
     fixed 定位、above 用 bottom 锚卡片上方，below 翻到下方；origin 动画从卡片侧生长 */
  .cpop {
    position: fixed; z-index: 60;
    background: #2a2a2e; border-radius: 10px;
    padding: 16px 16px 10px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.55);
    transform-origin: center bottom;
    animation: cpop-in 120ms ease-out;
  }
  .cpop--below { transform-origin: center top; }
  @keyframes cpop-in {
    from { opacity: 0; transform: scale(0.92); }
  }
  .cpop__arrow {
    position: absolute; bottom: -5px;
    width: 12px; height: 12px;
    background: #2a2a2e;
    transform: rotate(45deg); border-radius: 2px;
  }
  .cpop--below .cpop__arrow { top: -5px; bottom: auto; }
  .cpop__badge {
    position: absolute; top: 8px; right: 10px;
    font-size: 8px; font-weight: 700; letter-spacing: 0.5px;
    color: #a0a0a0;
    border: 1px solid rgba(255, 255, 255, 0.22); border-radius: 4px; padding: 1px 5px;
  }
  .cpop__frame { display: block; margin: 0 auto; border: 0; background: transparent; }
  .cpop__meta {
    display: flex; align-items: baseline; gap: 8px;
    margin-top: 8px; padding-top: 8px;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
  }
  .cpop__cn { font-size: 11px; color: #e8e8e8; font-weight: 600; }
  .cpop__src { font-size: 9px; color: #86868b; }
  .cpop__dim { margin-left: auto; font-size: 9px; color: #86868b; font-family: "SF Mono", Menlo, monospace; }
</style>
