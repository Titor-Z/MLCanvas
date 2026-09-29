<script lang="ts">
  interface Props {
    oncancel: () => void;
    oncreate: (name: string) => void;
  }

  let { oncancel, oncreate }: Props = $props();

  let name = $state("");
  const valid = $derived(name.trim().length > 0);

  function submit() {
    if (valid) oncreate(name.trim());
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
  <div class="dialog__hint">创建后会打开项目画布，在 Agent 里描述你的第一个界面即可。</div>
  <div class="dialog__actions">
    <button class="btn btn--ghost" onclick={oncancel}>取消</button>
    <button class="btn btn--primary" disabled={!valid} onclick={submit}>创建</button>
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
  .dialog__hint {
    margin-top: 10px;
    font-size: 11px; letter-spacing: 0.01em; line-height: 1.5;
    color: var(--text-3);
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
