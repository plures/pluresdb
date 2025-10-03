<script lang="ts">
  import { onMount } from "svelte";
  export let items: any[] = [];
  export let itemHeight = 32;
  export let height = 360;

  let container: HTMLDivElement;
  let scrollTop = 0;
  $: total = items.length * itemHeight;
  $: startIndex = Math.max(0, Math.floor(scrollTop / itemHeight) - 5);
  $: endIndex = Math.min(items.length, Math.ceil((scrollTop + height) / itemHeight) + 5);
  $: visible = items.slice(startIndex, endIndex);
  function onScroll() {
    scrollTop = container.scrollTop;
  }
</script>

<div bind:this={container} class="vlist" style={`height:${height}px`} on:scroll={onScroll}>
  <div style={`height:${total}px; position:relative`}>
    <div style={`position:absolute; top:${startIndex * itemHeight}px; left:0; right:0`}>
      <slot {visible} {startIndex}></slot>
    </div>
  </div>
</div>

<style>
  .vlist {
    overflow: auto;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
  }
</style>
