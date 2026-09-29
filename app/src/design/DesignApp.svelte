<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { designStore } from "./designStore.svelte.js";
  import type { BoardChange } from "./designStore.svelte.js";
  import TitleBar from "./components/TitleBar.svelte";
  import ToolRail from "./components/ToolRail.svelte";
  import NavPanel from "./components/NavPanel.svelte";
  import Canvas from "./components/Canvas.svelte";
  import Inspector from "./components/Inspector.svelte";

  onMount(async () => {
    await designStore.init();
    const stopChanged = await listen<{ changes: BoardChange[] }>("artboards-changed", (e) => {
      designStore.onChanged(e.payload?.changes ?? []);
    });
    const stopProject = await listen<string>("project-opened", (e) => {
      designStore.project = e.payload;
      designStore.fitted = false;
      designStore.refresh();
    });
    return () => {
      stopChanged();
      stopProject();
    };
  });
</script>

<div
  class="design"
  class:panel-closed={!designStore.panelOpen}
  class:inspector-closed={!designStore.inspectorOpen}
>
  <TitleBar />
  <ToolRail />
  <NavPanel />
  <Canvas />
  {#if designStore.inspectorOpen}
    <Inspector />
  {/if}
</div>

<style>
  .design {
    display: grid;
    grid-template-rows: 44px 1fr;
    grid-template-columns: 44px 210px 1fr 264px;
    grid-template-areas:
      "titlebar titlebar titlebar titlebar"
      "tools nav canvas inspector";
    height: 100%;
    background: var(--canvas);
  }
  .design.panel-closed {
    grid-template-columns: 44px 0px 1fr 264px;
  }
  .design.inspector-closed {
    grid-template-columns: 44px 210px 1fr 0px;
  }
  .design.panel-closed.inspector-closed {
    grid-template-columns: 44px 0px 1fr 0px;
  }
</style>
