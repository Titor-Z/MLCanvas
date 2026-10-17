<script lang="ts">
  interface Props {
    name: string;
    artboards: number;
    oncancel: () => void;
    onconfirm: (name: string) => void;
  }

  let { name, artboards, oncancel, onconfirm }: Props = $props();

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") oncancel();
    if (e.key === "Enter") onconfirm(name);
  }
</script>

<svelte:window onkeydown={onkeydown} />

<div class="scrim" data-id="scrim" onclick={oncancel}></div>
<div class="dialog" data-id="delete-project-dialog" role="dialog" aria-label="删除项目">
  <div class="dialog__title">删除项目？</div>
  <div class="dialog__hint">
    将删除「{name}」和它的 {artboards} 个画板，删除后无法从项目列表恢复。
  </div>
  <div class="dialog__actions">
    <button class="btn btn--ghost" onclick={oncancel}>取消</button>
    <button class="btn btn--danger" data-id="btn-delete" onclick={() => onconfirm(name)}>删除</button>
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
  .dialog__hint {
    margin-top: 12px;
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
  .btn--danger { background: #e81123; color: #fff; }
  .btn--danger:hover { background: #f5232f; }
</style>
