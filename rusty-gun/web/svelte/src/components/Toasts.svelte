<script lang="ts">
  import { toasts, dismiss } from '../lib/toasts'
</script>

<div class="toasts">
  {#each $toasts as t}
    <div class="toast {t.kind}">
      <span>{t.text}</span>
      <button class="secondary" on:click={() => dismiss(t.id)}>×</button>
    </div>
  {/each}
 </div>

<style>
  .toasts { position: fixed; right: 12px; bottom: 12px; display: flex; flex-direction: column; gap: 8px; z-index: 999 }
  .toast { display:flex; align-items:center; gap: 8px; padding: 8px 12px; border-radius: 8px; background: var(--pico-card-background-color); box-shadow: 0 2px 6px rgba(0,0,0,.15) }
  .toast.success { border-left: 4px solid #32a852 }
  .toast.error { border-left: 4px solid #d64545 }
  .toast.info { border-left: 4px solid #2a7bd6 }
  .toast button { padding: 0 6px }
</style>

