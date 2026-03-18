<script lang="ts">
  import type { Snippet } from "svelte";
  let { items = [], itemHeight = 32, height = 360, children }: {
    items: unknown[];
    itemHeight?: number;
    height?: number;
    children?: Snippet<[{ visible: unknown[]; startIndex: number }]>;
  } = $props();

  let container = $state<HTMLDivElement | undefined>(undefined);
  let scrollTop = $state(0);

  const total = $derived(items.length * itemHeight);
  const startIndex = $derived(Math.max(0, Math.floor(scrollTop / itemHeight) - 5));
  const endIndex = $derived(Math.min(items.length, Math.ceil((scrollTop + height) / itemHeight) + 5));
  const visible = $derived(items.slice(startIndex, endIndex));

  function onScroll() {
    if (container) scrollTop = container.scrollTop;
  }
</script>

<div bind:this={container} class="vlist" style={`height:${height}px`} onscroll={onScroll}>
  <div style={`height:${total}px; position:relative`}>
    <div style={`position:absolute; top:${startIndex * itemHeight}px; left:0; right:0`}>
      {#if children}
        {@render children({ visible, startIndex })}
      {/if}
    </div>
  </div>
</div>

<style>
  .vlist {
    overflow: auto;
    border: 1px solid var(--color-border, #ddd);
    border-radius: var(--radius-sm, 4px);
  }
</style>
