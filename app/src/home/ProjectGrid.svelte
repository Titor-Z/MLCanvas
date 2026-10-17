<script lang="ts">
  import type { ProjectInfo } from "../lib/services/projectService.js";
  import { shortenPath } from "../lib/services/projectService.js";

  interface Props {
    projects: ProjectInfo[];
    view: "grid" | "list";
    onnew: () => void;
    onopen: (name: string) => void;
    ondelete: (name: string) => void;
  }

  let { projects, view, onnew, onopen, ondelete }: Props = $props();

  function relativeTime(secs: number): string {
    if (!secs) return "—";
    const days = Math.floor((Date.now() / 1000 - secs) / 86400);
    if (days <= 0) return "今天";
    if (days === 1) return "昨天";
    if (days < 7) return `${days} 天前`;
    if (days < 30) return `${Math.floor(days / 7)} 周前`;
    return `${Math.floor(days / 30)} 个月前`;
  }
</script>

{#if view === "grid"}
  <div class="recents" data-id="recents">
    <button class="proj proj--new" data-id="proj-new" onclick={onnew}>
      <div class="proj__new"><svg viewBox="0 0 16 16"><path d="M8 3v10M3 8h10"/></svg></div>
      <div class="proj__name">新建项目</div>
    </button>
    {#each projects as p (p.name)}
      <button class="proj" onclick={() => onopen(p.name)} title={shortenPath(p.path)}>
        <div class="proj__thumb">
          <i style="background:linear-gradient(135deg,#4b9eff,#7a5cff)"></i>
          <span
            class="proj__del"
            data-id="proj-del"
            role="button"
            tabindex="-1"
            title="删除项目"
            onclick={(e) => { e.stopPropagation(); ondelete(p.name); }}
          >
            <svg viewBox="0 0 16 16"><path d="M3 5h10M6.5 5V3.5h3V5M5 5l.6 8h4.8L11 5"/></svg>
          </span>
        </div>
        <div class="proj__name">{p.name}</div>
        <div class="proj__time">{relativeTime(p.updated_at)}</div>
      </button>
    {/each}
  </div>
{:else}
  <div class="recents recents--list" data-id="recents">
    <button class="prow prow--new" onclick={onnew}>
      <div class="prow__thumb prow__new-thumb">
        <svg viewBox="0 0 16 16"><path d="M8 3v10M3 8h10"/></svg>
      </div>
      <div class="prow__meta"><div class="prow__name">新建项目</div></div>
    </button>
    {#each projects as p (p.name)}
      <button class="prow" onclick={() => onopen(p.name)}>
        <div class="prow__thumb"><i style="background:linear-gradient(135deg,#4b9eff,#7a5cff)"></i></div>
        <div class="prow__meta">
          <div class="prow__name">{p.name}</div>
          <div class="prow__path">{shortenPath(p.path)}</div>
        </div>
        <div class="prow__right">
          <span class="prow__count">{p.artboards} 个画板</span>
          <span class="prow__time">{relativeTime(p.updated_at)}</span>
        </div>
        <span
          class="prow__del"
          data-id="proj-del"
          role="button"
          tabindex="-1"
          title="删除项目"
          onclick={(e) => { e.stopPropagation(); ondelete(p.name); }}
        >
          <svg viewBox="0 0 16 16"><path d="M3 5h10M6.5 5V3.5h3V5M5 5l.6 8h4.8L11 5"/></svg>
        </span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .recents { display: grid; grid-template-columns: repeat(3, 1fr); gap: 16px; }

  .proj {
    display: flex; flex-direction: column; align-items: stretch; justify-content: flex-start;
    text-align: left;
    border: 0;
    background: none;
    padding: 0;
  }
  .proj__thumb, .proj__new {
    height: 118px;
    background: var(--card);
    border: 1px solid var(--line);
    border-radius: var(--radius-card);
    transition: border-color 160ms ease-out, transform 100ms ease-out;
  }
  .proj__thumb { position: relative; overflow: hidden; }
  .proj__thumb i { position: absolute; inset: 12px; border-radius: 4px; }
  .proj:hover .proj__thumb, .proj:hover .proj__new { border-color: rgba(255, 255, 255, 0.22); }
  .proj:active .proj__thumb, .proj:active .proj__new { transform: scale(0.98); }
  .proj__new { display: grid; place-items: center; }
  .proj__new svg { width: 18px; height: 18px; fill: none; stroke: var(--text-2); stroke-width: 1.5; stroke-linecap: round; }
  .proj__name { margin-top: 10px; font-size: 13px; font-weight: 600; line-height: 1.3; }
  .proj__time { margin-top: 2px; font-size: 11px; letter-spacing: 0.01em; line-height: 1.3; color: var(--text-3); }
  /* 删除：悬停卡片时出现在缩略图右上角 */
  .proj__del {
    position: absolute; top: 8px; right: 8px;
    width: 24px; height: 24px;
    display: grid; place-items: center;
    background: rgba(0, 0, 0, 0.55);
    border-radius: 6px;
    color: var(--text-2);
    opacity: 0;
    transition: opacity 160ms ease-out;
  }
  .proj__del svg { width: 12px; height: 12px; fill: none; stroke: currentColor; stroke-width: 1.4; stroke-linecap: round; }
  .proj:hover .proj__del { opacity: 1; }
  .proj__del:hover { color: #ff6b6b; }

  /* 列表视图 */
  .recents--list { display: block; max-width: 640px; }
  .prow {
    display: flex; align-items: center; gap: 14px;
    width: 100%;
    padding: 10px 12px;
    border-radius: var(--radius-btn);
    text-align: left;
    transition: background-color 160ms ease-out;
  }
  .prow:hover { background: var(--card); }
  .prow:active { opacity: 0.85; }
  .prow__thumb {
    width: 54px; height: 40px; flex-shrink: 0;
    border-radius: 4px;
    border: 1px solid var(--line);
    position: relative; overflow: hidden;
    background: var(--card);
  }
  .prow__thumb i { position: absolute; inset: 5px; border-radius: 2px; }
  .prow__new-thumb { display: grid; place-items: center; }
  .prow__new-thumb svg { width: 14px; height: 14px; fill: none; stroke: var(--text-2); stroke-width: 1.5; stroke-linecap: round; }
  .prow__meta { flex: 1; }
  .prow__name { font-size: 13px; font-weight: 600; line-height: 1.3; }
  .prow__path { margin-top: 2px; font-size: 11px; letter-spacing: 0.01em; line-height: 1.3; color: var(--text-3); }
  .prow__time { font-size: 11px; letter-spacing: 0.01em; line-height: 1.3; color: var(--text-3); }
  .prow__right { display: flex; flex-direction: column; align-items: flex-end; gap: 2px; }
  .prow__count { font-size: 11px; color: var(--text-3); font-family: "SF Mono", Menlo, monospace; }
  /* 删除：悬停行时出现在行尾 */
  .prow__del {
    width: 26px; height: 26px; flex-shrink: 0;
    display: grid; place-items: center;
    border-radius: 6px;
    color: var(--text-2);
    opacity: 0;
    transition: opacity 160ms ease-out;
  }
  .prow__del svg { width: 13px; height: 13px; fill: none; stroke: currentColor; stroke-width: 1.4; stroke-linecap: round; }
  .prow:hover .prow__del { opacity: 1; }
  .prow__del:hover { color: #ff6b6b; }
</style>
