<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import { EditorState } from '@codemirror/state'
  import { EditorView, keymap, highlightActiveLine, drawSelection } from '@codemirror/view'
  import { defaultKeymap, history, historyKeymap } from '@codemirror/commands'
  import { json } from '@codemirror/lang-json'
  import { oneDark } from '@codemirror/theme-one-dark'

  export let value = ''
  export let dark = false
  export let onChange: (v: string) => void = () => {}
  let container: HTMLDivElement
  let view: EditorView | null = null

  function buildExtensions() {
    const base = [
      keymap.of([...defaultKeymap, ...historyKeymap]),
      history(),
      highlightActiveLine(),
      drawSelection(),
      json(),
      EditorView.updateListener.of((u) => {
        if (u.docChanged) {
          const doc = u.state.doc.toString()
          onChange(doc)
        }
      }),
    ]
    if (dark) base.push(oneDark)
    return base
  }

  function createEditor() {
    const state = EditorState.create({
      doc: value,
      extensions: buildExtensions(),
    })
    view = new EditorView({ state, parent: container })
  }

  $: if (view) {
    // Update theme if dark changes
    const ext = buildExtensions()
    view.dispatch({ effects: EditorState.reconfigure.of(ext) as any })
  }

  $: if (view && value !== view.state.doc.toString()) {
    // External value change
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: value } })
  }

  onMount(() => { createEditor() })
  onDestroy(() => { if (view) view.destroy() })
</script>

<div bind:this={container} class="editor"></div>

<style>
  .editor { border: 1px solid var(--pico-muted-border-color); border-radius: 8px; min-height: 280px; }
</style>

