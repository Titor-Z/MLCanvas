<script lang="ts">
  import type { IconName } from "./icons.js";

  let { name, size = 16, class: klass = "" }: { name: IconName; size?: number; class?: string } = $props();
</script>

<svg
  class="i {klass}"
  viewBox="0 0 16 16"
  width={size}
  height={size}
  aria-hidden="true"
>
  {#if name === "select"}
    <path d="M4 2.5 12.4 8.6 8.7 9.2 6.6 13.2Z" />
  {:else if name === "hand"}
    <path d="M5.5 8.5V4.5a1 1 0 0 1 2 0v3m0-1.5a1 1 0 0 1 2 0V8m0-1a1 1 0 0 1 2 0v4.5a3.5 3.5 0 0 1-3.5 3.5h-.5a3 3 0 0 1-3-3V9.5" />
  {:else if name === "annotate"}
    <path d="M10.8 2.9a1.7 1.7 0 0 1 2.4 2.4L5.4 13.1l-3 .5.5-3Z" />
  {:else if name === "design"}
    <path d="M2.5 4.5V3.5a1 1 0 0 1 1-1h1M11.5 2.5h1a1 1 0 0 1 1 1v1M13.5 11.5v1a1 1 0 0 1-1 1h-1M4.5 13.5h-1a1 1 0 0 1-1-1v-1" />
    <circle cx="8" cy="8" r="1.4" fill="currentColor" stroke="none" />
  {:else if name === "resource"}
    <path d="M8 2 10.2 4.6 8 7.2 5.8 4.6ZM4.6 7.4 6.8 10 4.6 12.6 2.4 10ZM11.4 7.4 13.6 10 11.4 12.6 9.2 10Z" />
  {:else if name === "market"}
    <path d="M2.8 6 4.2 3h7.6L13.2 6Z" />
    <path d="M2.8 6h10.4v6.6a1 1 0 0 1-1 1H3.8a1 1 0 0 1-1-1Z" />
    <path d="M6.5 13.6v-3.2h3v3.2" />
  {:else if name === "board"}
    <rect x="3.2" y="2.5" width="9.6" height="11" rx="1.3" />
    <path d="M5.4 5.6h5.2M5.4 8h5.2M5.4 10.4h3" />
  {:else if name === "chat"}
    <rect x="2.5" y="3.2" width="11" height="7.8" rx="2.2" />
    <path d="M5.8 11v2.6l3-2.6" />
  {:else if name === "send"}
    <path d="M8 12.5V4M4.2 7.6 8 3.8l3.8 3.8" />
  {:else if name === "stop"}
    <rect x="5" y="5" width="6" height="6" rx="1.6" fill="currentColor" stroke="none" />
  {:else if name === "plus"}
    <path d="M8 3.5v9M3.5 8h9" />
  {:else if name === "close"}
    <path d="M4.2 4.2 11.8 11.8M11.8 4.2 4.2 11.8" />
  {:else if name === "check"}
    <path d="M3.6 8.4 6.5 11.3 12.4 4.9" />
  {:else if name === "clock"}
    <circle cx="8" cy="8" r="5.2" />
    <path d="M8 5.2V8l1.9 1.2" />
  {:else if name === "chevron"}
    <path d="M9.6 3.8 5.6 8l4 4.2" />
  {:else if name === "caret-down"}
    <path d="M4 6.5 8 10.5 12 6.5" />
  {:else if name === "retry"}
    <path d="M12.7 8a4.7 4.7 0 1 1-1.6-3.5" />
    <path d="M12.9 2.5v3.1h-3.1" />
  {:else if name === "copy"}
    <rect x="5.6" y="5.6" width="7.9" height="7.9" rx="1.8" />
    <path d="M10.4 5.6V4.1a1.8 1.8 0 0 0-1.8-1.8H4.1a1.8 1.8 0 0 0-1.8 1.8v4.5a1.8 1.8 0 0 0 1.8 1.8h1.4" />
  {:else if name === "compact"}
    <path d="M2.6 4.6h10.8M2.6 8h7M2.6 11.4h4.6" />
  {:else if name === "doc"}
    <rect x="3.2" y="2.5" width="9.6" height="11" rx="1.3" />
    <path d="M5.4 6h5.2M5.4 8.6h5.2" />
  {:else if name === "spark"}
    <path d="M8 2.2 9.5 6.5 13.8 8 9.5 9.5 8 13.8 6.5 9.5 2.2 8 6.5 6.5Z" />
  {:else if name === "spinner"}
    <path d="M8 2.6a5.4 5.4 0 1 1-5.4 5.4" />
  {/if}
</svg>

<style>
  .i {
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.5;
    stroke-linecap: round;
    stroke-linejoin: round;
    flex-shrink: 0;
  }
</style>
