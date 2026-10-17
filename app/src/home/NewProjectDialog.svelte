<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { shortenPath } from "../lib/services/projectService.js";

  interface Props {
    oncancel: () => void;
    oncreate: (name: string, dir?: string) => void;
    error?: string;
  }

  let { oncancel, oncreate, error = "" }: Props = $props();

  let name = $state("");
  /** 目录地址栏显示文本（~ 形式），提交时原样传给后端展开 */
  let path = $state("");
  /** 用户手动编辑过路径或浏览选择后，名称→路径联动停止 */
  let pathTouched = $state(false);
  let browsing = $state(false);
  const valid = $derived(name.trim().length > 0);

  // 名称 → 路径实时联动
  $effect(() => {
    if (!pathTouched) path = `~/YuCanvas/${name.trim()}`;
  });

  async function browse() {
    if (browsing) return;
    browsing = true;
    try {
      const dir = await invoke<string | null>("pick_directory");
      if (dir) {
        path = shortenPath(dir);
        pathTouched = true;
        // 所选目录即项目目录，目录名与项目名保持一致（后端校验同名）
        const base = dir.split("/").filter(Boolean).pop() ?? "";
        if (base) name = base;
      }
    } finally {
      browsing = false;
    }
  }

  function submit() {
    if (valid) oncreate(name.trim(), path.trim() || undefined);
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") oncancel();
    if (e.key === "Enter") submit();
  }
</script>

<svelte:window onkeydown={onkeydown} />

<div class="scrim" data-id="scrim" onclick={oncancel}></div>
<div class="dialog" data-id="new-project-dialog" role="dialog" aria-label="新建项目">
  <div class="dialog__title">新建项目</div>
  <input
    class="dialog__input"
    placeholder="项目名称"
    bind:value={name}
    autofocus
  />
  <div class="dialog__pathrow" data-id="dlg-path">
    <input
      class="dialog__path"
      title="项目目录"
      bind:value={path}
      oninput={() => (pathTouched = true)}
      spellcheck="false"
    />
    <button class="dialog__browse" data-id="dlg-browse" title="选择目录" onclick={browse} disabled={browsing}>
      <svg viewBox="0 0 16 16"><path d="M1.5 4.5A1.5 1.5 0 0 1 3 3h3l1.5 1.5H13A1.5 1.5 0 0 1 14.5 6v5A1.5 1.5 0 0 1 13 12.5H3A1.5 1.5 0 0 1 1.5 11z"/></svg>
    </button>
  </div>
  {#if error}
    <div class="dialog__error">{error}</div>
  {:else}
    <div class="dialog__hint">
      创建后会打开项目画布，在 Agent 里描述你的第一个界面即可。Agent 会从项目信息里拿到这个目录。
    </div>
  {/if}
  <div class="dialog__actions">
    <button class="btn btn--ghost" onclick={oncancel}>取消</button>
    <button class="btn btn--primary" disabled={!valid || browsing} onclick={submit}>创建</button>
  </div>
</div>

<style>
  .scrim {
    position: fixed; inset: 0;
    background: rgba(0, 0, 0, 0.45);
  }
  .dialog {
    position: fixed; top: 50%; left: 50%;
    transform: translate(-50%, calc(-50% - 20px));
    width: 400px;
    background: var(--chrome);
    border: 1px solid var(--line);
    border-radius: 12px;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.45);
    padding: 20px;
  }
  .dialog__title { font-size: 15px; font-weight: 700; }
  .dialog__input {
    margin-top: 16px;
    width: 100%;
    height: 36px;
    padding: 0 12px;
  }
  /* 目录地址栏：左路径输入 + 右浏览按钮（默认随名称联动） */
  .dialog__pathrow {
    margin-top: 10px;
    height: 36px;
    display: flex; align-items: center;
    padding: 0 4px 0 0;
    background: var(--card);
    border: 1px solid var(--line);
    border-radius: var(--radius-btn);
  }
  .dialog__pathrow:focus-within { border-color: var(--accent); }
  .dialog__path {
    flex: 1;
    height: 100%;
    padding: 0 0 0 12px;
    border: 0;
    background: none;
    font-family: "SF Mono", Menlo, monospace;
    font-size: 12px;
    color: var(--text-2);
  }
  .dialog__path:focus { outline: none; color: var(--text); }
  .dialog__browse {
    width: 28px; height: 28px; flex-shrink: 0;
    display: grid; place-items: center;
    border: 0;
    border-radius: 4px;
    background: none;
    color: var(--text-2);
  }
  .dialog__browse svg { width: 15px; height: 15px; fill: none; stroke: currentColor; stroke-width: 1.4; stroke-linecap: round; stroke-linejoin: round; }
  .dialog__browse:hover { background: rgba(255, 255, 255, 0.08); color: var(--text); }
  .dialog__browse:disabled { opacity: 0.5; }
  .dialog__hint {
    margin-top: 10px;
    font-size: 11px; letter-spacing: 0.01em; line-height: 1.5;
    color: var(--text-3);
  }
  .dialog__error {
    margin-top: 10px;
    font-size: 11px; letter-spacing: 0.01em; line-height: 1.5;
    color: #ff6b6b;
  }
  .dialog__actions {
    display: flex; justify-content: flex-end; gap: 8px;
    margin-top: 20px;
  }
  .btn {
    height: 32px; padding: 0 16px;
    display: flex; align-items: center;
    border-radius: var(--radius-btn);
    font-size: 13px; font-weight: 600;
  }
  .btn--ghost { color: var(--text-2); }
  .btn--ghost:hover { background: var(--card); color: var(--text); }
  .btn--primary { background: var(--accent); color: #fff; }
  .btn--primary:disabled { opacity: 0.4; cursor: default; }
</style>
