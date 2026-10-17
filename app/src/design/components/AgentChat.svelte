<script lang="ts">
  import { onMount } from "svelte";
  import { designStore } from "../designStore.svelte.js";
  import { agentPresence } from "../../lib/agentPresence.svelte.js";
  import { scrollFade } from "../../lib/scrollFade.js";
  import { renderMarkdown } from "../../lib/markdown.js";
  import { Icon } from "../../lib/icons.js";

  let draft = $state("");
  let listEl = $state<HTMLElement | null>(null);
  let inputEl = $state<HTMLTextAreaElement | null>(null);
  let activityOpen = $state(false);
  let usageOpen = $state(false);
  let cmdIndex = $state(0);
  /** 展开查看摘要的 compact 行 id */
  let openCompact = $state<number | null>(null);

  const entries = $derived(designStore.chat);
  const hints = ["画一个音乐 App 的登录页", "把「首页」改成暗色", "为这个项目定一套设计规范"];

  const COMMANDS = [
    { name: "/compact", label: "压缩上下文", desc: "把前面的对话整理成工作纪要，画板不受影响" },
    { name: "/new", label: "新对话", desc: "清空这一栏，开始一个新话题" },
    { name: "/conventions", label: "装载设计手册", desc: "让助手重新读入项目规范与工作手册" },
  ];
  /** 输入框以 / 开头且还没敲空格时弹出命令面板 */
  const cmdQuery = $derived(/^\/\S*$/.test(draft) ? draft.slice(1).toLowerCase() : null);
  const cmdItems = $derived(
    cmdQuery === null
      ? []
      : COMMANDS.filter((c) => c.name.slice(1).startsWith(cmdQuery)),
  );

  /** 上下文环四档色 */
  const ring = $derived.by(() => {
    const u = designStore.usage;
    if (!u || !u.limit) return null;
    const ratio = Math.min(1, u.used / u.limit);
    const tone = ratio >= 0.95 ? "full" : ratio >= 0.85 ? "high" : ratio >= 0.6 ? "mid" : "low";
    // 圆环周长 2πr，r = 9
    const circumference = 2 * Math.PI * 9;
    return { ratio, tone, dash: `${(circumference * ratio).toFixed(1)} ${circumference.toFixed(1)}` };
  });

  function fmtK(n: number): string {
    return n >= 1000 ? `${(n / 1000).toFixed(1)}K` : String(n);
  }

  function submit() {
    const text = draft;
    draft = "";
    cmdIndex = 0;
    designStore.sendChat(text);
  }

  function runCommand(name: string) {
    draft = "";
    cmdIndex = 0;
    if (name === "/compact") designStore.compactChat();
    else if (name === "/new") designStore.clearChat();
    else if (name === "/conventions") designStore.sendChat("先调用 get_conventions 重新读入工作手册，再读一次项目规范，确认你按规范工作。");
  }

  function stop() {
    designStore.cancelChat();
  }

  function openSettings() {
    designStore.openSettings("model");
  }

  // 输入框随内容长高（上限 200px）
  $effect(() => {
    void draft;
    const el = inputEl;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(200, Math.max(88, el.scrollHeight))}px`;
  });

  function onKeydown(e: KeyboardEvent) {
    // 命令面板优先接管上下键与回车
    if (cmdItems.length > 0) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        cmdIndex = (cmdIndex + 1) % cmdItems.length;
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        cmdIndex = (cmdIndex + cmdItems.length - 1) % cmdItems.length;
        return;
      }
      if (e.key === "Enter" && !e.isComposing) {
        e.preventDefault();
        runCommand(cmdItems[Math.min(cmdIndex, cmdItems.length - 1)].name);
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        draft = "";
        return;
      }
    }
    // 回车发送，Shift+回车换行；输入法组词过程不触发
    if (e.key === "Enter" && !e.isComposing && !e.shiftKey) {
      e.preventDefault();
      submit();
      return;
    }
    if (e.key === "Escape" && designStore.chatBusy) {
      e.preventDefault();
      stop();
    }
  }

  // Esc 随时可停（焦点不在输入框时也算）；点空白收起浮层
  onMount(() => {
    const onEsc = (e: KeyboardEvent) => {
      if (!(e.target as HTMLElement)?.closest?.("input, textarea")) {
        if (e.key === "Escape" && designStore.chatBusy) stop();
        usageOpen = false;
      }
    };
    window.addEventListener("keydown", onEsc);
    return () => window.removeEventListener("keydown", onEsc);
  });

  // 新消息 / 流式输出时贴底
  $effect(() => {
    void entries.length;
    void entries[entries.length - 1]?.text.length;
    listEl?.scrollTo({ top: listEl.scrollHeight });
  });
</script>

<div class="chat" data-id="agent-chat">
  <div class="chat__head">
    <b>设计助手</b>
    <span
      class="chat__state"
      class:chat__state--busy={agentPresence.display.tone === "busy"}
      class:chat__state--off={agentPresence.display.tone === "off"}
    >
      <i></i>{agentPresence.display.text}
    </span>
    {#if !designStore.chatBusy && entries.length > 0}
      <button class="chat__act" title="新对话" onclick={() => designStore.clearChat()}>
        <Icon name="plus" />
      </button>
    {/if}
  </div>

  <div class="chat__scroll" bind:this={listEl} use:scrollFade>
    {#if !designStore.configured}
      <div class="empty">
        <div class="empty__mark"><Icon name="spark" /></div>
        <div class="empty__title">接入你的模型</div>
        <div class="empty__desc">连接你喜欢的模型服务后，设计助手就能直接为你画画板、改样式。</div>
        <button class="empty__btn" onclick={openSettings}>接入模型</button>
      </div>
    {:else if entries.length === 0}
      <div class="chat__inner">
        <div class="msg msg--ai"><b>你好，我是设计助手。</b><br />描述你想要的界面，我会直接在画布上画出来；也可以让我改现有画板、定义项目风格。</div>
        <div class="hints">
          {#each hints as hint}
            <button class="hint" onclick={() => designStore.sendChat(hint)}>
              <Icon name="spark" />{hint}
            </button>
          {/each}
        </div>
      </div>
    {:else}
      <div class="chat__inner">
        {#each entries as entry (entry.id)}
          {#if entry.kind === "user"}
            <div class="msg msg--user">{entry.text}</div>
          {:else if entry.kind === "tool"}
            <div class="step" class:step--run={entry.toolState === "run"}>
              <span class="step__icon"><Icon name={entry.toolState === "run" ? "spinner" : "check"} /></span>
              {entry.text}
            </div>
          {:else if entry.kind === "step-end"}
            <div class="step step--muted">
              <span class="step__icon"><Icon name="stop" /></span>
              {entry.text}
            </div>
          {:else if entry.kind === "compact"}
            <div class="compact">
              <button class="compact__label" onclick={() => (openCompact = openCompact === entry.id ? null : entry.id)}>
                <Icon name="chevron" class={openCompact === entry.id ? "flip" : ""} />
                已压缩前 {entry.compactCount ?? 0} 轮 · 摘要 {entry.text.length} 字
              </button>
              {#if openCompact === entry.id}
                <div class="compact__body">{entry.text}</div>
              {/if}
            </div>
          {:else}
            <div class="msg msg--ai" class:msg--error={entry.error} class:msg--streaming={entry.streaming}>
              {#if entry.error}
                {entry.text}
              {:else}
                <div class="md">{@html renderMarkdown(entry.text)}</div>
              {/if}
            </div>
          {/if}
        {/each}
      </div>
    {/if}
  </div>

  <button class="chat__activity" onclick={() => (activityOpen = !activityOpen)}>
    <Icon name="chevron" class={activityOpen ? "flip" : ""} />
    活动记录
    <em>{designStore.logs.length}</em>
  </button>
  {#if activityOpen}
    <div class="log" use:scrollFade>
      {#each designStore.logs as entry (entry.time + entry.text)}
        <div class="log__item">
          <span class="log__time">{entry.time}</span>
          <span class="log__text">{entry.text}</span>
        </div>
      {/each}
      {#if designStore.logs.length === 0}
        <div class="log__empty">还没有活动</div>
      {/if}
    </div>
  {/if}

  <div class="composer-wrap">
    {#if cmdItems.length > 0}
      <div class="palette">
        <div class="palette__head">命令</div>
        {#each cmdItems as cmd, i}
          <button class="cmd" class:cmd--on={i === cmdIndex} onclick={() => runCommand(cmd.name)}>
            <span class="cmd__icon"><Icon name={cmd.name === "/compact" ? "compact" : cmd.name === "/new" ? "plus" : "doc"} /></span>
            <span class="cmd__body">
              <span class="cmd__name">{cmd.label}</span>
              <span class="cmd__desc">{cmd.desc}</span>
            </span>
            <span class="cmd__key">{cmd.name}</span>
          </button>
        {/each}
      </div>
    {/if}

    {#if usageOpen && ring && designStore.usage}
      <div class="pop">
        <div class="pop__title">
          <b>上下文</b>
          <em>{designStore.chatModel || "当前模型"}</em>
        </div>
        <div class="pop__bar"><div class="pop__fill {ring.tone}" style="width: {(ring.ratio * 100).toFixed(0)}%"></div></div>
        <div class="pop__rows">
          <div class="pop__row">已用<em>{fmtK(designStore.usage.used)} / {fmtK(designStore.usage.limit!)}</em></div>
          <div class="pop__row pop__row--muted">其中输入<em>{fmtK(designStore.usage.input)}</em></div>
          <div class="pop__row pop__row--muted">其中输出<em>{fmtK(designStore.usage.output)}</em></div>
        </div>
        <div class="pop__note">对话越长，每次请求带的历史越多。压缩会把前面的对话整理成一段工作纪要，画板与规范不受影响。</div>
        <button class="pop__act" onclick={() => { usageOpen = false; designStore.compactChat(); }}>
          <Icon name="compact" />压缩上下文
        </button>
      </div>
    {/if}

    <div class="composer" class:composer--focus={draft.length > 0 || usageOpen}>
      <textarea
        class="composer__text"
        rows="1"
        placeholder="描述你想要的界面…"
        bind:this={inputEl}
        bind:value={draft}
        onkeydown={onKeydown}
        disabled={designStore.chatBusy}
      ></textarea>
      <div class="composer__bar">
        <button class="tool" title="插入命令（/）" onclick={() => { draft = "/"; inputEl?.focus(); }}>
          <Icon name="plus" />
        </button>
        {#if designStore.chatModel}
          <button class="chip" onclick={openSettings} title="切换模型">
            {designStore.chatModel}
            <Icon name="caret-down" />
          </button>
        {/if}
        <span class="composer__spacer"></span>
        {#if ring}
          <button
            class="ring ring--{ring.tone}"
            class:ring--on={usageOpen}
            title="上下文 {Math.round(ring.ratio * 100)}%"
            onclick={() => (usageOpen = !usageOpen)}
          >
            <svg viewBox="0 0 24 24">
              <circle class="track" cx="12" cy="12" r="9" />
              <circle class="val" cx="12" cy="12" r="9" stroke-dasharray={ring.dash} />
            </svg>
          </button>
        {/if}
        {#if designStore.chatBusy}
          <button
            class="send send--stop"
            class:send--busy={designStore.chatStopping}
            title={designStore.chatStopping ? "正在停止…" : "停止（Esc）"}
            disabled={designStore.chatStopping}
            onclick={stop}
          >
            <Icon name={designStore.chatStopping ? "spinner" : "stop"} />
          </button>
        {:else}
          <button class="send" title="发送" disabled={!draft.trim()} onclick={submit}>
            <Icon name="send" />
          </button>
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  .chat {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .chat__head {
    display: flex; align-items: center; gap: 8px;
    padding: 13px 16px 11px;
    flex-shrink: 0;
  }
  .chat__head b { font-size: 13px; font-weight: 600; letter-spacing: 0.1px; }
  .chat__state {
    display: flex; align-items: center; gap: 5px;
    font-size: 10.5px; color: var(--text-2);
  }
  .chat__state i {
    width: 6px; height: 6px; border-radius: 50%;
    background: var(--ok);
    box-shadow: 0 0 0 3px rgba(52, 199, 89, 0.14);
  }
  .chat__state--off i { background: var(--text-3); box-shadow: none; }
  .chat__state--busy i {
    background: var(--accent);
    box-shadow: 0 0 0 3px rgba(75, 158, 255, 0.16);
    animation: breathe 1.8s ease-in-out infinite;
  }
  @keyframes breathe { 50% { box-shadow: 0 0 0 5px rgba(75, 158, 255, 0.06); } }
  .chat__act {
    margin-left: auto;
    width: 24px; height: 24px;
    display: grid; place-items: center;
    border-radius: 6px;
    color: var(--text-3);
    transition: background 140ms ease-out, color 140ms ease-out;
  }
  .chat__act:hover { background: rgba(255, 255, 255, 0.07); color: var(--text-2); }

  .chat__scroll { flex: 1; min-height: 0; overflow-y: auto; padding: 2px 16px 12px; }
  .chat__inner { display: flex; flex-direction: column; gap: 14px; padding-top: 4px; }

  .msg--user {
    align-self: flex-end;
    max-width: 84%;
    padding: 10px 14px;
    background: linear-gradient(180deg, #38414d, #313945);
    border: 1px solid rgba(255, 255, 255, 0.09);
    border-radius: 14px 14px 4px 14px;
    font-size: 13px; line-height: 1.65;
    white-space: pre-wrap; word-break: break-word;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.25);
  }
  .msg--ai { font-size: 13.5px; line-height: 1.72; word-break: break-word; }
  .msg--error { color: #ff6b6b; white-space: pre-wrap; }
  .msg--streaming .md :global(> :last-child)::after {
    content: "";
    display: inline-block;
    width: 6px; height: 15px;
    margin-left: 2px; vertical-align: -1px;
    background: var(--accent);
    animation: blink 1s steps(2) infinite;
  }
  @keyframes blink { 50% { opacity: 0; } }

  /* Markdown 回复排版 */
  .md :global(*) + :global(*) { margin-top: 9px; }
  .md :global(h3), .md :global(h4), .md :global(h5), .md :global(h6) { font-size: 13.5px; font-weight: 600; line-height: 1.5; }
  .md :global(ul), .md :global(ol) { padding-left: 17px; }
  .md :global(li) + :global(li) { margin-top: 4px; }
  .md :global(li)::marker { color: var(--text-3); }
  .md :global(code) {
    font-family: "SF Mono", Menlo, monospace;
    font-size: 12px;
    padding: 2px 5px;
    background: rgba(255, 255, 255, 0.08);
    border-radius: 4px;
    color: #d9e8ff;
  }
  .md :global(pre.md-code) {
    padding: 11px 12px;
    background: #1b1b1f;
    border: 1px solid var(--line);
    border-radius: 8px;
    overflow-x: auto;
  }
  .md :global(pre.md-code) :global(code) { padding: 0; background: none; color: #cfd6e4; line-height: 1.65; }
  .md :global(blockquote) { padding-left: 11px; border-left: 2px solid rgba(255, 255, 255, 0.14); color: var(--text-2); }
  .md :global(a) { color: var(--accent); text-decoration: none; }
  .md :global(a):hover { text-decoration: underline; }
  .md :global(hr) { border: 0; border-top: 1px solid var(--line); }
  .md :global(strong) { font-weight: 600; }

  /* 过程行 */
  .step {
    display: flex; align-items: center; gap: 8px;
    font-size: 12px; color: var(--text-2);
  }
  .step__icon { width: 14px; height: 14px; flex-shrink: 0; display: grid; place-items: center; color: var(--text-3); }
  .step--run { color: var(--text); }
  .step--run .step__icon { color: var(--accent); }
  .step--run .step__icon :global(svg) { animation: spin 0.9s linear infinite; }
  .step--muted { color: var(--text-3); }
  @keyframes spin { to { transform: rotate(360deg); } }

  /* 压缩标记 */
  .compact { display: flex; flex-direction: column; gap: 7px; }
  .compact__label {
    display: flex; align-items: center; gap: 6px;
    font-size: 11.5px; color: var(--text-3);
    text-align: left;
  }
  .compact__label :global(svg) { transition: transform 160ms ease-out; }
  .compact__label :global(svg.flip) { transform: rotate(90deg); }
  .compact__body {
    padding: 9px 11px;
    background: rgba(255, 255, 255, 0.04);
    border-left: 2px solid var(--line);
    border-radius: 0 6px 6px 0;
    font-size: 12.5px; line-height: 1.65; color: var(--text-2);
    white-space: pre-wrap;
  }

  /* 空态 */
  .empty {
    height: 100%;
    display: flex; flex-direction: column;
    align-items: center; justify-content: center;
    text-align: center;
    padding: 0 26px;
  }
  .empty__mark {
    width: 44px; height: 44px;
    border-radius: 12px;
    background: var(--accent-soft);
    color: var(--accent);
    display: grid; place-items: center;
    margin-bottom: 14px;
  }
  .empty__title { font-size: 13px; font-weight: 600; margin-bottom: 6px; }
  .empty__desc { font-size: 12px; color: var(--text-2); line-height: 1.6; margin-bottom: 16px; }
  .empty__btn {
    height: 28px; padding: 0 14px;
    background: var(--accent);
    border-radius: 7px;
    font-size: 12px; font-weight: 600; color: #fff;
  }
  .empty__btn:active { transform: scale(0.97); }

  .hints { display: flex; flex-direction: column; gap: 7px; margin-top: 2px; }
  .hint {
    display: flex; align-items: center; gap: 8px;
    padding: 9px 12px;
    background: var(--field);
    border-radius: 9px;
    font-size: 12px; color: var(--text-2);
    text-align: left;
    transition: background 150ms ease-out, color 150ms ease-out;
  }
  .hint :global(svg) { color: var(--text-3); flex-shrink: 0; }
  .hint:hover { background: rgba(255, 255, 255, 0.07); color: var(--text); }
  .hint:active { transform: scale(0.98); }

  /* 活动记录 */
  .chat__activity {
    flex-shrink: 0;
    display: flex; align-items: center; gap: 6px;
    padding: 9px 16px;
    border-top: 1px solid var(--line);
    font-size: 11.5px; color: var(--text-2);
    text-align: left;
  }
  .chat__activity :global(svg) { color: var(--text-3); transition: transform 160ms ease-out; }
  .chat__activity :global(svg.flip) { transform: rotate(90deg); }
  .chat__activity em { margin-left: auto; font-style: normal; font-size: 10.5px; color: var(--text-3); }
  .log {
    flex-shrink: 0;
    max-height: 140px; overflow-y: auto;
    padding: 8px 16px;
    border-top: 1px solid var(--line);
    display: grid; gap: 6px;
    align-content: start;
  }
  .log__item { display: flex; gap: 8px; align-items: baseline; font-size: 11.5px; line-height: 1.45; }
  .log__time { color: var(--text-3); flex-shrink: 0; font-family: "SF Mono", Menlo, monospace; font-size: 10.5px; }
  .log__text { color: var(--text-2); }
  .log__empty { font-size: 11.5px; color: var(--text-3); }

  /* 输入区 */
  .composer-wrap {
    flex-shrink: 0;
    position: relative;
    padding: 0 16px 16px;
    background: linear-gradient(rgba(36, 36, 41, 0), var(--panel) 24%);
  }
  .composer {
    background: var(--raise);
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 14px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.34), 0 1px 0 rgba(255, 255, 255, 0.08) inset;
    display: flex; flex-direction: column;
    transition: border-color 160ms ease-out, box-shadow 160ms ease-out;
  }
  .composer--focus {
    border-color: rgba(75, 158, 255, 0.55);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.34), 0 0 0 3px rgba(75, 158, 255, 0.14), 0 1px 0 rgba(255, 255, 255, 0.08) inset;
  }
  .composer__text {
    min-height: 88px;
    max-height: 200px;
    padding: 13px 14px 10px;
    font-size: 13px; line-height: 1.65;
    color: var(--text);
    background: none; border: 0; outline: none;
    resize: none;
    overflow-y: auto;
  }
  .composer__text::placeholder { color: var(--text-3); }
  .composer__text:disabled { color: var(--text-2); }
  .composer__bar { display: flex; align-items: center; gap: 7px; padding: 0 12px 12px; }
  .composer__spacer { flex: 1; }

  .tool {
    width: 28px; height: 28px; flex-shrink: 0;
    display: grid; place-items: center;
    border-radius: 8px;
    color: var(--text-2);
    transition: background 140ms ease-out, color 140ms ease-out;
  }
  .tool:hover { background: rgba(255, 255, 255, 0.06); color: var(--text); }
  .chip {
    display: flex; align-items: center; gap: 6px;
    height: 28px; padding: 0 10px;
    background: rgba(255, 255, 255, 0.06);
    border-radius: 8px;
    font-size: 11.5px; color: var(--text-2);
    transition: background 140ms ease-out, color 140ms ease-out;
  }
  .chip:hover { background: rgba(255, 255, 255, 0.1); color: var(--text); }
  .chip :global(svg) { width: 10px; height: 10px; color: var(--text-3); }

  /* 上下文环 */
  .ring {
    width: 28px; height: 28px; flex-shrink: 0;
    display: grid; place-items: center;
    border-radius: 50%;
    transition: background 140ms ease-out;
  }
  .ring:hover, .ring--on { background: rgba(255, 255, 255, 0.07); }
  .ring :global(svg) { width: 18px; height: 18px; transform: rotate(-90deg); }
  .ring :global(circle) { fill: none; stroke-width: 2.4; }
  .ring :global(.track) { stroke: rgba(255, 255, 255, 0.12); }
  .ring :global(.val) { stroke-linecap: round; transition: stroke 200ms ease-out; }
  .ring--low :global(.val) { stroke: #8e8e99; }
  .ring--mid :global(.val) { stroke: var(--accent); }
  .ring--high :global(.val) { stroke: #e0a03a; }
  .ring--full :global(.val) { stroke: #e05252; }

  .send {
    width: 28px; height: 28px; flex-shrink: 0;
    display: grid; place-items: center;
    border-radius: 50%;
    background: var(--accent); color: #fff;
    box-shadow: 0 1px 0 rgba(255, 255, 255, 0.22) inset, 0 2px 5px rgba(0, 0, 0, 0.28);
    transition: background 140ms ease-out, transform 100ms ease-out, opacity 140ms ease-out;
  }
  .send :global(svg) { width: 16px; height: 16px; }
  .send:disabled { opacity: 0.35; }
  .send:active { transform: scale(0.94); }
  .send--stop {
    background: rgba(255, 255, 255, 0.1);
    color: #e2e2e8;
    box-shadow: 0 1px 0 rgba(255, 255, 255, 0.08) inset;
  }
  .send--stop :global(svg) { fill: currentColor; stroke: none; }
  .send--busy { opacity: 0.6; }
  .send--busy :global(svg) { fill: none; stroke: currentColor; animation: spin 0.9s linear infinite; }

  /* 命令面板 */
  .palette {
    position: absolute; left: 16px; right: 16px; bottom: calc(100% - 6px);
    background: #2a2a2e;
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 12px;
    box-shadow: 0 18px 44px rgba(0, 0, 0, 0.5), 0 1px 0 rgba(255, 255, 255, 0.07) inset;
    padding: 5px;
    z-index: 2;
  }
  .palette__head { padding: 6px 9px 5px; font-size: 10.5px; color: var(--text-3); }
  .cmd {
    display: flex; align-items: center; gap: 9px;
    width: 100%;
    padding: 8px 9px;
    border-radius: 7px;
    text-align: left;
  }
  .cmd--on { background: rgba(255, 255, 255, 0.08); }
  .cmd__icon {
    width: 20px; height: 20px; flex-shrink: 0;
    display: grid; place-items: center;
    border-radius: 5px;
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-2);
  }
  .cmd--on .cmd__icon { background: rgba(75, 158, 255, 0.2); color: #a8cdff; }
  .cmd__body { display: grid; gap: 2px; min-width: 0; }
  .cmd__name { font-size: 12px; color: var(--text); }
  .cmd__desc { font-size: 10.5px; color: var(--text-3); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .cmd__key { margin-left: auto; font-size: 10px; color: var(--text-3); font-family: "SF Mono", Menlo, monospace; }

  /* 上下文浮层 */
  .pop {
    position: absolute; right: 16px; bottom: calc(100% - 6px);
    width: 270px;
    background: #2a2a2e;
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 12px;
    box-shadow: 0 18px 44px rgba(0, 0, 0, 0.5), 0 1px 0 rgba(255, 255, 255, 0.07) inset;
    padding: 12px 13px 11px;
    z-index: 2;
  }
  .pop__title { display: flex; align-items: baseline; gap: 6px; margin-bottom: 9px; }
  .pop__title b { font-size: 12.5px; font-weight: 600; }
  .pop__title em {
    margin-left: auto; font-style: normal;
    font-size: 10.5px; color: var(--text-3);
    max-width: 150px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .pop__bar { height: 5px; border-radius: 3px; background: rgba(255, 255, 255, 0.1); overflow: hidden; margin-bottom: 9px; }
  .pop__fill { height: 100%; border-radius: 3px; background: #8e8e99; transition: width 220ms ease-out; }
  .pop__fill.mid { background: var(--accent); }
  .pop__fill.high { background: #e0a03a; }
  .pop__fill.full { background: #e05252; }
  .pop__rows { display: grid; gap: 5px; margin-bottom: 10px; }
  .pop__row { display: flex; align-items: center; font-size: 11px; color: var(--text-2); }
  .pop__row em { margin-left: auto; font-style: normal; color: var(--text); font-variant-numeric: tabular-nums; }
  .pop__row--muted em { color: var(--text-2); }
  .pop__note {
    font-size: 10.5px; color: var(--text-3); line-height: 1.55;
    padding: 8px 0 9px;
    border-top: 1px solid var(--line);
  }
  .pop__act {
    display: flex; align-items: center; justify-content: center; gap: 6px;
    width: 100%; height: 28px;
    border-radius: 7px;
    background: rgba(224, 160, 58, 0.16);
    color: #e8bc72;
    font-size: 11.5px; font-weight: 600;
    transition: background 140ms ease-out;
  }
  .pop__act:hover { background: rgba(224, 160, 58, 0.24); }
  .pop__act:active { transform: scale(0.98); }
</style>
