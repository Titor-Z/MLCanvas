<script lang="ts">
  import { designStore } from "../designStore.svelte.js";

  const selected = $derived(designStore.selected);
</script>

<aside class="inspector" data-id="inspector">
  <div class="inspector__section" data-id="panel-board">
    <div class="inspector__title"><span>画板</span></div>
    {#if selected}
      <div class="fieldrow">
        <span class="fieldrow__label">名称</span>
        <input class="fieldrow__input" value={selected.name} readonly />
      </div>
      <div class="fieldrow">
        <span class="fieldrow__label">尺寸</span>
        <input class="fieldrow__input fieldrow__input--num" value={selected.width} readonly />
        <span class="fieldrow__x">×</span>
        <input class="fieldrow__input fieldrow__input--num" value={selected.height} readonly />
      </div>
    {:else}
      <div class="inspector__empty">在画布中选择一个画板</div>
    {/if}
  </div>

  <div class="inspector__section" data-id="panel-agent">
    <div class="inspector__title">
      <span>Agent 协作</span>
      <span class="inspector__state"><i></i>空闲</span>
    </div>
    <div class="log">
      {#if designStore.logs.length === 0}
        <div class="log__empty">监听项目文件变化中…</div>
      {:else}
        {#each designStore.logs as entry (entry.time + entry.text)}
          <div class="log__item">
            <span class="log__time">{entry.time}</span>
            <span class="log__text">{entry.text}</span>
          </div>
        {/each}
      {/if}
    </div>
  </div>

  <div class="inspector__spacer"></div>
  <div class="inspector__footer">
    <input class="inspector__input" placeholder="给 Agent 写张便签…" disabled />
    <div class="inspector__send" title="V2 提供">
      <svg viewBox="0 0 13 13"><path d="M1.5 6.5 L11.5 1.5 L8 11.5 L5.5 7z"/></svg>
    </div>
  </div>
</aside>

<style>
  .inspector {
    grid-area: inspector;
    background: var(--chrome);
    border-left: 1px solid var(--line);
    display: flex; flex-direction: column;
    overflow: hidden;
  }
  .inspector__section { padding: 12px 14px; border-bottom: 1px solid var(--line); }
  .inspector__title {
    font-size: 10px; letter-spacing: 1px; text-transform: uppercase;
    color: var(--text-3); margin-bottom: 10px;
    display: flex; align-items: center; justify-content: space-between;
  }
  .inspector__state {
    display: flex; align-items: center; gap: 5px;
    letter-spacing: 0; text-transform: none;
    font-size: 10px; color: var(--text-2);
  }
  .inspector__state i { width: 6px; height: 6px; border-radius: 50%; background: var(--ok); }
  .fieldrow { display: flex; align-items: center; gap: 8px; height: 30px; margin-bottom: 6px; }
  .fieldrow__label { width: 30px; flex-shrink: 0; color: var(--text-2); }
  .fieldrow__input {
    flex: 1;
    height: 28px; padding: 0 9px;
    font-size: 12px;
  }
  .fieldrow__input--num { font-family: "SF Mono", Menlo, monospace; font-size: 11px; }
  .fieldrow__x { color: var(--text-3); }
  .inspector__empty { font-size: 12px; color: var(--text-3); }

  .log { display: grid; gap: 8px; }
  .log__empty { font-size: 11px; color: var(--text-3); }
  .log__item { display: flex; gap: 8px; align-items: baseline; font-size: 11px; line-height: 1.4; }
  .log__time { color: var(--text-3); flex-shrink: 0; font-family: "SF Mono", Menlo, monospace; font-size: 10px; }
  .log__text { color: var(--text-2); }

  .inspector__spacer { flex: 1; }
  .inspector__footer {
    padding: 10px 14px;
    border-top: 1px solid var(--line);
    display: flex; gap: 8px;
  }
  .inspector__input { flex: 1; height: 30px; padding: 0 10px; font-size: 11px; }
  .inspector__input:disabled { color: var(--text-3); }
  .inspector__send {
    width: 30px; height: 30px; flex-shrink: 0;
    display: grid; place-items: center;
    background: var(--field); border-radius: var(--radius-btn);
  }
  .inspector__send svg { width: 13px; height: 13px; fill: none; stroke: var(--text-3); stroke-width: 1.6; }
</style>
