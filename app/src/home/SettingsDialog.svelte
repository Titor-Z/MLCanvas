<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import {
    agentService,
    type AgentSettingsInfo,
    type ModelCatalogItem,
    type ModelConfig,
  } from "../lib/services/agentService.js";

  interface Props {
    oncancel: () => void;
    /** 保存成功后回调（设计窗口据此刷新「已接入」状态） */
    onsaved?: () => void;
    /** 打开时显示的标签页 */
    initialTab?: Tab;
  }

  let { oncancel, onsaved, initialTab = "general" }: Props = $props();

  type Tab = "general" | "model" | "data" | "about";
  let tab = $state<Tab>(untrack(() => initialTab));

  let settings = $state<AgentSettingsInfo | null>(null);
  let loadError = $state("");
  /** 模型 tab：list = 配置列表；form = 添加/编辑表单 */
  let mode = $state<"list" | "form">("list");

  // ── 通用：默认项目位置 ──
  let defaultParent = $state("");
  let locationError = $state("");

  // ── 表单状态 ──
  let editingId = $state<string | null>(null);
  let provider = $state("DeepSeek");
  let protocol = $state<"openai" | "anthropic">("openai");
  let baseUrl = $state("");
  let apiKey = $state("");
  let model = $state("");
  let testing = $state(false);
  let testOk = $state<boolean | null>(null);
  let testMsg = $state("");
  let formError = $state("");

  // ── 模型库下拉 ──
  let pickerOpen = $state(false);
  let pickerLoading = $state(false);
  let pickerError = $state("");
  let catalog = $state<ModelCatalogItem[]>([]);
  let catalogSource = $state("");
  /** catalog 对应的接口地址（换服务商后重新拉取） */
  let catalogFor = $state("");
  let query = $state("");
  let manual = $state(false);

  const providers: { name: string; protocol: "openai" | "anthropic"; base_url: string }[] = [
    { name: "DeepSeek", protocol: "openai", base_url: "https://api.deepseek.com/v1" },
    { name: "智谱 GLM", protocol: "openai", base_url: "https://open.bigmodel.cn/api/paas/v4" },
    { name: "Kimi", protocol: "openai", base_url: "https://api.moonshot.cn/v1" },
    { name: "通义千问", protocol: "openai", base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1" },
    { name: "Anthropic", protocol: "anthropic", base_url: "https://api.anthropic.com" },
    { name: "自定义", protocol: "openai", base_url: "" },
  ];

  const active = $derived(settings?.configs.find((c) => c.id === settings?.current) ?? null);
  const valid = $derived(
    model.trim().length > 0 && (protocol === "anthropic" || baseUrl.trim().length > 0),
  );
  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return catalog;
    return catalog.filter((m) => m.id.toLowerCase().includes(q) || m.name.toLowerCase().includes(q));
  });

  onMount(load);

  async function load() {
    try {
      settings = await agentService.getSettings();
      defaultParent = settings.default_parent;
    } catch (e) {
      loadError = clean(e);
    }
  }

  function clean(e: unknown): string {
    return String(e).replace(/^.*Error:\s*/, "").trim() || "操作失败";
  }

  /** 把绝对路径里 HOME 前缀缩成 ~ 显示 */
  function shorten(p: string): string {
    return p.replace(/^\/Users\/[^/]+/, "~").replace(/^\/home\/[^/]+/, "~");
  }

  function switchTab(t: Tab) {
    tab = t;
    mode = "list";
    pickerOpen = false;
  }

  // ── 模型配置列表操作 ──
  function editConfig(c: ModelConfig) {
    editingId = c.id;
    provider = c.provider || "自定义";
    protocol = c.protocol === "anthropic" ? "anthropic" : "openai";
    baseUrl = c.base_url;
    apiKey = c.api_key;
    model = c.model;
    testOk = null;
    testMsg = "";
    formError = "";
    pickerOpen = false;
    manual = false;
    tab = "model";
    mode = "form";
  }

  function addConfig() {
    editingId = null;
    provider = "DeepSeek";
    protocol = "openai";
    baseUrl = providers[0].base_url;
    apiKey = "";
    model = "";
    testOk = null;
    testMsg = "";
    formError = "";
    pickerOpen = false;
    manual = false;
    mode = "form";
  }

  function pickProvider(p: (typeof providers)[number]) {
    provider = p.name;
    protocol = p.protocol;
    baseUrl = p.base_url;
    testOk = null;
    pickerOpen = false;
  }

  async function setCurrent(id: string) {
    try {
      await agentService.setCurrent(id);
      await load();
      onsaved?.();
    } catch (e) {
      loadError = clean(e);
    }
  }

  async function removeConfig(id: string) {
    try {
      await agentService.deleteConfig(id);
      await load();
      onsaved?.();
    } catch (e) {
      loadError = clean(e);
    }
  }

  // ── 表单操作 ──
  async function test() {
    if (testing || !valid) return;
    testing = true;
    testOk = null;
    testMsg = "";
    try {
      await agentService.testConnection({ protocol, base_url: baseUrl.trim(), api_key: apiKey.trim(), model: model.trim() });
      testOk = true;
    } catch (e) {
      testOk = false;
      testMsg = clean(e);
    } finally {
      testing = false;
    }
  }

  async function save() {
    if (!valid) return;
    formError = "";
    try {
      await agentService.saveConfig({
        id: editingId ?? undefined,
        provider: provider.trim() || "自定义",
        protocol,
        base_url: baseUrl.trim(),
        api_key: apiKey.trim(),
        model: model.trim(),
      });
      await load();
      onsaved?.();
      mode = "list";
      pickerOpen = false;
    } catch (e) {
      formError = clean(e);
    }
  }

  // ── 模型库 ──
  async function openPicker() {
    if (pickerOpen) {
      pickerOpen = false;
      return;
    }
    pickerOpen = true;
    query = "";
    const forUrl = baseUrl.trim();
    if (catalog.length > 0 && catalogFor === forUrl) return;
    catalogFor = forUrl;
    catalog = [];
    pickerLoading = true;
    pickerError = "";
    try {
      const res = await agentService.modelCatalog(forUrl);
      catalog = res.models;
      catalogSource = res.source;
    } catch (e) {
      pickerError = clean(e);
    } finally {
      pickerLoading = false;
    }
  }

  function choose(m: ModelCatalogItem) {
    model = m.id;
    pickerOpen = false;
    testOk = null;
  }

  // ── 通用：更换默认位置 ──
  async function changeLocation() {
    locationError = "";
    try {
      const dir = await invoke<string | null>("pick_directory");
      if (!dir) return;
      await agentService.setDefaultParent(dir);
      await load();
    } catch (e) {
      locationError = clean(e);
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      if (pickerOpen) pickerOpen = false;
      else if (tab === "model" && mode === "form") mode = "list";
      else oncancel();
    }
  }

  function fmtContext(n: number): string {
    if (!n) return "";
    if (n >= 1_000_000) return `${Math.round(n / 100_000) / 10}M`;
    if (n >= 1000) return `${Math.round(n / 1000)}K`;
    return String(n);
  }
</script>

<svelte:window onkeydown={onkeydown} />

<div class="scrim" data-id="scrim" role="presentation" onclick={oncancel}></div>
<div class="dialog" data-id="settings-dialog" role="dialog" aria-label="设置">
  <div class="dialog__head">
    <div class="dialog__title">设置</div>
    <button class="dialog__close" title="关闭" onclick={oncancel}>
      <svg viewBox="0 0 10 10"><path d="M1.5 1.5 8.5 8.5M8.5 1.5 1.5 8.5" /></svg>
    </button>
  </div>

  <div class="dialog__body">
    <nav class="nav">
      <button class="nav__item" class:nav__item--active={tab === "general"} onclick={() => switchTab("general")}>通用</button>
      <button class="nav__item" class:nav__item--active={tab === "model"} onclick={() => switchTab("model")}>模型</button>
      <button class="nav__item" class:nav__item--active={tab === "data"} onclick={() => switchTab("data")}>数据</button>
      <button class="nav__item" class:nav__item--active={tab === "about"} onclick={() => switchTab("about")}>关于</button>
      <div class="nav__spacer"></div>
      <div class="nav__foot">{settings?.version ?? ""}</div>
    </nav>

    {#if tab === "general"}
      <div class="content">
        <div class="section">
          <div class="section__label">外观</div>
          <div class="row">
            <div class="row__col"><div class="row__label">界面语言</div></div>
            <div class="row__side">
              <div class="seg">
                <div class="seg__item seg__item--active">简体中文</div>
                <div class="seg__item seg__item--soon">English</div>
              </div>
            </div>
          </div>
          <div class="row">
            <div class="row__col">
              <div class="row__label">主题</div>
              <div class="row__desc">亮色在准备中</div>
            </div>
            <div class="row__side">
              <div class="seg">
                <div class="seg__item seg__item--active">暗色</div>
                <div class="seg__item seg__item--soon">亮色</div>
              </div>
            </div>
          </div>
        </div>

        <div class="section">
          <div class="section__label">项目</div>
          <div class="row">
            <div class="row__col">
              <div class="row__label">新建项目默认位置</div>
              <div class="row__desc">新建时可以随时更换目录</div>
            </div>
            <div class="row__side">
              <span class="path">{shorten(defaultParent || "~/YuCanvas")}</span>
              <button class="btn" onclick={changeLocation}>更换</button>
            </div>
          </div>
        </div>
        {#if locationError}<div class="error">{locationError}</div>{/if}
      </div>

    {:else if tab === "model"}
      <div class="content">
        {#if mode === "list"}
          <div class="statusline">
            <i class:statusline__dot--off={!active}></i>
            {#if active}
              当前 <b>{active.provider || "自定义"}</b> · {active.model}
            {:else}
              尚未接入模型
            {/if}
          </div>

          <div class="section">
            <div class="section__label">模型配置</div>
            {#each settings?.configs ?? [] as c (c.id)}
              <div
                class="row row--click"
                onclick={() => editConfig(c)}
                onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); editConfig(c); } }}
                role="button"
                tabindex="0"
              >
                <div class="row__col">
                  <div class="cfg">
                    <i class="dot" class:dot--on={c.id === settings?.current}></i>
                    <span class="cfg__vendor">{c.provider || "自定义"}</span>
                    <span class="cfg__model">{c.model}</span>
                  </div>
                </div>
                <div class="row__side">
                  {#if c.id === settings?.current}
                    <span class="badge">当前</span>
                  {:else}
                    <button class="btn" onclick={(e) => { e.stopPropagation(); setCurrent(c.id); }}>设为当前</button>
                  {/if}
                  <button class="btn btn--ghost" onclick={(e) => { e.stopPropagation(); removeConfig(c.id); }}>删除</button>
                </div>
              </div>
            {/each}
            <button class="addrow" onclick={addConfig}>
              <svg viewBox="0 0 12 12"><path d="M6 1.5v9M1.5 6h9" /></svg>
              添加模型配置
            </button>
          </div>

          <div class="section">
            <div class="section__label">说明</div>
            <div class="row row--last">
              <div class="row__col">
                <div class="row__desc" style="margin-top:0;">可以同时配置多个厂商与模型，点击某一行可修改密钥或换模型；对话面板顶部可随时切换当前使用的模型。</div>
              </div>
            </div>
          </div>
          {#if loadError}<div class="error">{loadError}</div>{/if}

        {:else}
          <div class="form-body" class:form-body--dim={pickerOpen}>
          <div class="formhead">
            <b>{editingId ? "编辑模型配置" : "添加模型配置"}</b>
            <button class="cancel" onclick={() => (mode = "list")}>取消</button>
          </div>

          <div class="section">
            <div class="section__label">服务</div>
            <div class="row">
              <div class="row__col">
                <div class="row__label">服务商</div>
                <div class="row__desc">自定义可接任何兼容服务，包括本地模型</div>
              </div>
              <div class="row__side">
                <div class="seg">
                  {#each providers as p (p.name)}
                    <button class="seg__item" class:seg__item--active={provider === p.name} onclick={() => pickProvider(p)}>{p.name}</button>
                  {/each}
                </div>
              </div>
            </div>
            <div class="row">
              <div class="row__col">
                <div class="row__label">接口地址</div>
                <div class="row__desc">选服务商后自动带出，无需手填</div>
              </div>
              <div class="row__side row__side--grow">
                <input class="input input--mono" spellcheck="false" placeholder={protocol === "anthropic" ? "https://api.anthropic.com" : "https://api.deepseek.com/v1"} bind:value={baseUrl} />
              </div>
            </div>
            <div class="row">
              <div class="row__col">
                <div class="row__label">密钥</div>
                <div class="row__desc">只存在本机，不会上传</div>
              </div>
              <div class="row__side row__side--grow">
                <input class="input input--mono" type="password" spellcheck="false" placeholder="sk-…" bind:value={apiKey} />
              </div>
            </div>
          </div>

          <div class="section">
            <div class="section__label">模型</div>
            <div class="row">
              <div class="row__col">
                <div class="row__label">模型</div>
                <div class="row__desc">来自公共模型库，实时更新 · 标注上下文长度</div>
              </div>
              <div class="row__side">
                <button class="modelbtn" class:modelbtn--open={pickerOpen} onclick={openPicker}>
                  {model || "选择模型…"}
                  <span class="caret"></span>
                </button>
              </div>
            </div>
          </div>

          <div class="foot">
            <button class="test" onclick={test} disabled={!valid || testing}>{testing ? "测试中…" : "测试连接"}</button>
            {#if testOk === true}
              <span class="okdot"><i></i>连接成功</span>
            {:else if testOk === false}
              <span class="okdot okdot--fail">{testMsg}</span>
            {/if}
            <div class="foot__spacer"></div>
            <button class="done" onclick={save} disabled={!valid}>完成</button>
          </div>
          {#if formError}<div class="error">{formError}</div>{/if}
          </div>

          {#if pickerOpen}
            <div class="picker">
              {#if manual}
                <div class="picker__manual">
                  <input class="picker__manual-input" spellcheck="false" placeholder="输入模型 ID" bind:value={model} />
                  <button class="btn btn--ghost" onclick={() => (manual = false)}>返回列表</button>
                </div>
              {:else}
                <div class="picker__search">
                  <svg viewBox="0 0 12 12"><circle cx="5" cy="5" r="3.4" /><path d="M7.6 7.6 10.4 10.4" /></svg>
                  <input spellcheck="false" placeholder="搜索模型…" bind:value={query} />
                </div>
                <div class="picker__list">
                  {#if pickerLoading}
                    <div class="picker__empty">正在读取模型库…</div>
                  {:else if pickerError}
                    <div class="picker__empty">{pickerError}</div>
                  {:else if filtered.length === 0}
                    <div class="picker__empty">{catalog.length === 0 ? "该服务商暂无模型清单，可手动输入" : "没有匹配的模型"}</div>
                  {:else}
                    {#each filtered as m (m.id)}
                      <button class="pick" class:pick--active={model === m.id} onclick={() => choose(m)}>
                        <span class="pick__id">{m.id}</span>
                        {#if m.reasoning}<span class="pick__tag">推理</span>{/if}
                        {#if m.vision}<span class="pick__tag">视觉</span>{/if}
                        {#if m.context}<span class="pick__ctx">{fmtContext(m.context)}</span>{/if}
                      </button>
                    {/each}
                  {/if}
                </div>
              {/if}
              {#if !manual}
                <button class="picker__custom" onclick={() => { manual = true; }}>
                  <svg viewBox="0 0 12 12"><path d="M6 1.5v9M1.5 6h9" /></svg>
                  手动输入模型 ID
                </button>
              {/if}
              {#if catalogSource && !manual}
                <div class="picker__src"><i></i>公共模型库实时更新 · {catalogSource}</div>
              {/if}
            </div>
          {/if}
        {/if}
      </div>

    {:else if tab === "data"}
      <div class="content">
        <div class="section">
          <div class="section__label">存储</div>
          <div class="row">
            <div class="row__col">
              <div class="row__label">项目位置</div>
              <div class="row__desc">项目文件保存在本机，默认放在下列目录，可随时更换</div>
            </div>
            <div class="row__side"><span class="path">{shorten(defaultParent || "~/YuCanvas")}</span></div>
          </div>
          <div class="row">
            <div class="row__col">
              <div class="row__label">模型配置</div>
              <div class="row__desc">密钥只存在本机，不会上传到任何服务器</div>
            </div>
          </div>
          <div class="row row--last">
            <div class="row__col">
              <div class="row__label">对话记录</div>
              <div class="row__desc">按项目保留在本机，随项目一起使用</div>
            </div>
          </div>
        </div>
      </div>

    {:else}
      <div class="content about">
        <div class="about__brand"><span class="yu">聿</span>Canvas</div>
        <div class="about__ver">版本 {settings?.version ?? "—"}</div>
        <div class="about__desc">Agent 优先的 UI 设计工具。向助手描述你想要的界面，它直接在画布上画出来。</div>
      </div>
    {/if}
  </div>
</div>

<style>
  .scrim { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.45); }
  .dialog {
    position: fixed; top: 50%; left: 50%;
    transform: translate(-50%, calc(-50% - 16px));
    width: 720px; height: 528px;
    background: var(--chrome);
    border: 1px solid var(--line);
    border-radius: 10px;
    box-shadow: 0 18px 50px rgba(0, 0, 0, 0.5);
    overflow: hidden;
    display: flex; flex-direction: column;
  }
  .dialog__head {
    height: 46px; flex-shrink: 0;
    display: flex; align-items: center;
    padding: 0 12px 0 20px;
    border-bottom: 1px solid var(--line);
  }
  .dialog__title { font-size: 13px; font-weight: 700; }
  .dialog__close {
    margin-left: auto; width: 26px; height: 26px;
    display: grid; place-items: center;
    border-radius: 6px; color: var(--text-3);
  }
  .dialog__close:hover { background: var(--card); color: var(--text); }
  .dialog__close svg { width: 10px; height: 10px; fill: none; stroke: currentColor; stroke-width: 1.4; stroke-linecap: round; }

  .dialog__body { flex: 1; min-height: 0; display: grid; grid-template-columns: 148px 1fr; }

  .nav { border-right: 1px solid var(--line); padding: 14px 10px; display: flex; flex-direction: column; gap: 1px; }
  .nav__item {
    height: 30px; padding: 0 10px;
    display: flex; align-items: center;
    border-radius: 6px;
    font-size: 12.5px; color: var(--text-2);
    text-align: left;
  }
  .nav__item:hover { color: var(--text); }
  .nav__item--active { background: var(--accent-soft); color: var(--accent); font-weight: 600; }
  .nav__spacer { flex: 1; }
  .nav__foot { padding: 6px 10px; font-size: 10.5px; color: var(--text-3); font-family: "SF Mono", Menlo, monospace; }

  .content { overflow-y: auto; padding: 22px 28px; position: relative; }
  .form-body--dim { opacity: 0.45; pointer-events: none; }

  .section { margin-bottom: 24px; }
  .section__label { font-size: 10px; letter-spacing: 1.2px; text-transform: uppercase; color: var(--text-3); margin-bottom: 4px; }
  .row { display: flex; align-items: center; gap: 16px; padding: 12px 0; border-bottom: 1px solid var(--line); }
  .row--last, .section .row:last-child { border-bottom: none; }
  .row--click { cursor: default; }
  .row__col { flex: 1; min-width: 0; }
  .row__label { font-size: 12.5px; }
  .row__desc { font-size: 11px; color: var(--text-3); margin-top: 3px; line-height: 1.5; }
  .row__side { display: flex; align-items: center; gap: 8px; flex-shrink: 0; }
  .row__side--grow { flex: 1; max-width: 320px; }

  .statusline { display: flex; align-items: center; gap: 7px; font-size: 11.5px; color: var(--text-2); padding-bottom: 18px; }
  .statusline i { width: 6px; height: 6px; border-radius: 50%; background: var(--ok); flex-shrink: 0; }
  .statusline i.statusline__dot--off { background: var(--text-3); }
  .statusline b { color: var(--text); font-weight: 600; }

  .seg { display: flex; gap: 2px; padding: 2px; background: var(--field); border-radius: 6px; }
  .seg__item { height: 24px; padding: 0 11px; display: flex; align-items: center; border-radius: 4px; font-size: 11.5px; color: var(--text-2); }
  .seg__item:hover { color: var(--text); }
  .seg__item--active { background: #3a3a3e; color: var(--text); font-weight: 600; }
  .seg__item--soon { color: var(--text-3); cursor: default; }

  .path { font-family: "SF Mono", Menlo, monospace; font-size: 11px; color: var(--text-2); }

  .input {
    width: 100%; height: 27px; padding: 0 10px;
    background: var(--field);
    border: 1px solid transparent;
    border-radius: 6px;
    font-size: 11.5px; color: var(--text);
  }
  .input:focus { border-color: var(--accent); }
  .input--mono { font-family: "SF Mono", Menlo, monospace; font-size: 11px; }

  .btn {
    height: 24px; padding: 0 10px;
    display: flex; align-items: center;
    background: var(--field);
    border-radius: 6px;
    font-size: 11px; color: var(--text-2);
    transition: color 150ms ease-out, transform 100ms ease-out;
  }
  .btn:hover { color: var(--text); }
  .btn:active { transform: scale(0.97); }
  .btn--ghost { background: none; color: var(--text-3); }
  .btn--ghost:hover { color: var(--text); }

  .badge {
    height: 20px; padding: 0 8px;
    display: flex; align-items: center;
    background: var(--accent-soft);
    border-radius: 10px;
    font-size: 10.5px; font-weight: 600; color: var(--accent);
  }

  .cfg { display: flex; align-items: center; gap: 10px; min-width: 0; }
  .cfg .dot { width: 6px; height: 6px; border-radius: 50%; flex-shrink: 0; background: var(--text-3); }
  .cfg .dot--on { background: var(--ok); }
  .cfg__vendor { font-size: 12.5px; font-weight: 600; flex-shrink: 0; }
  .cfg__model { font-family: "SF Mono", Menlo, monospace; font-size: 11px; color: var(--text-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  .addrow { display: flex; align-items: center; gap: 9px; padding: 12px 0; font-size: 12px; color: var(--text-3); }
  .addrow svg { width: 12px; height: 12px; fill: none; stroke: currentColor; stroke-width: 1.5; stroke-linecap: round; }
  .addrow:hover { color: var(--text); }

  .formhead { display: flex; align-items: center; padding-bottom: 14px; }
  .formhead b { font-size: 13px; font-weight: 700; }
  .formhead .cancel { margin-left: auto; font-size: 11.5px; color: var(--text-3); }
  .formhead .cancel:hover { color: var(--text); }

  .modelbtn {
    display: flex; align-items: center; gap: 8px;
    height: 27px; padding: 0 10px;
    background: var(--field); border-radius: 6px;
    font-size: 11.5px; color: var(--text);
    min-width: 218px; max-width: 260px;
  }
  .modelbtn:not(.modelbtn--open) { color: var(--text-2); }
  .modelbtn--open { background: var(--accent-soft); color: var(--accent); font-weight: 600; }
  .modelbtn .caret { margin-left: auto; width: 0; height: 0; border-left: 4px solid transparent; border-right: 4px solid transparent; border-top: 5px solid currentColor; }

  .foot { margin-top: auto; display: flex; align-items: center; gap: 10px; padding-top: 14px; border-top: 1px solid var(--line); }
  .test { height: 26px; padding: 0 12px; display: flex; align-items: center; background: var(--field); border-radius: 6px; font-size: 11.5px; color: var(--text); }
  .test:disabled { opacity: 0.4; }
  .okdot { display: flex; align-items: center; gap: 5px; font-size: 11px; color: var(--ok); }
  .okdot i { width: 5px; height: 5px; border-radius: 50%; background: var(--ok); flex-shrink: 0; }
  .okdot--fail { color: #ff6b6b; }
  .foot__spacer { flex: 1; }
  .done { height: 27px; padding: 0 16px; display: flex; align-items: center; background: var(--accent); border-radius: 6px; font-size: 11.5px; font-weight: 600; color: #fff; }
  .done:disabled { opacity: 0.4; }

  .error { margin-top: 10px; font-size: 11px; color: #ff6b6b; }

  .picker {
    position: absolute; right: 28px; top: 108px;
    width: 340px;
    background: #2a2a2e;
    border: 1px solid rgba(255, 255, 255, 0.09);
    border-radius: 9px;
    box-shadow: 0 14px 40px rgba(0, 0, 0, 0.55);
    overflow: hidden;
    z-index: 10;
  }
  .picker__search { margin: 9px; height: 27px; background: var(--field); border-radius: 6px; display: flex; align-items: center; gap: 7px; padding: 0 9px; }
  .picker__search svg { width: 11px; height: 11px; fill: none; stroke: var(--text-3); stroke-width: 1.4; stroke-linecap: round; flex-shrink: 0; }
  .picker__search input { flex: 1; height: 100%; font-size: 11px; background: none; border: 0; color: var(--text); }
  .picker__list { max-height: 218px; overflow-y: auto; padding: 0 5px 5px; }
  .pick { width: 100%; display: flex; align-items: baseline; gap: 8px; padding: 7px 8px; border-radius: 6px; text-align: left; }
  .pick:hover { background: rgba(255, 255, 255, 0.06); }
  .pick__id { font-family: "SF Mono", Menlo, monospace; font-size: 11px; color: var(--text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pick--active .pick__id { color: var(--accent); font-weight: 600; }
  .pick__tag { font-size: 9px; font-weight: 600; color: var(--accent); background: var(--accent-soft); border-radius: 3px; padding: 1px 4px; flex-shrink: 0; }
  .pick__ctx { margin-left: auto; flex-shrink: 0; font-family: "SF Mono", Menlo, monospace; font-size: 10px; color: var(--text-3); }
  .picker__empty { padding: 14px 13px; font-size: 11px; color: var(--text-3); }
  .picker__manual { display: flex; align-items: center; gap: 8px; padding: 9px; }
  .picker__manual-input { flex: 1; height: 27px; padding: 0 9px; background: var(--field); border-radius: 6px; font-size: 11px; font-family: "SF Mono", Menlo, monospace; color: var(--text); }
  .picker__custom { width: 100%; border-top: 1px solid var(--line); padding: 8px 13px 9px; font-size: 11.5px; color: var(--text-2); display: flex; align-items: center; gap: 8px; }
  .picker__custom:hover { color: var(--text); }
  .picker__custom svg { width: 12px; height: 12px; fill: none; stroke: currentColor; stroke-width: 1.4; stroke-linecap: round; }
  .picker__src { padding: 0 13px 9px; font-size: 10px; color: var(--text-3); display: flex; align-items: center; gap: 5px; }
  .picker__src i { width: 5px; height: 5px; border-radius: 50%; background: var(--ok); }

  .about { display: flex; flex-direction: column; align-items: center; justify-content: center; text-align: center; gap: 8px; }
  .about__brand { display: flex; align-items: center; gap: 2px; font-size: 22px; font-weight: 700; }
  .about__brand .yu { font-size: 28px; font-weight: 600; background: linear-gradient(135deg, #4b9eff, #7a5cff); -webkit-background-clip: text; background-clip: text; -webkit-text-fill-color: transparent; }
  .about__ver { font-size: 11px; color: var(--text-3); font-family: "SF Mono", Menlo, monospace; }
  .about__desc { max-width: 380px; font-size: 12px; color: var(--text-2); line-height: 1.7; margin-top: 6px; }
</style>
