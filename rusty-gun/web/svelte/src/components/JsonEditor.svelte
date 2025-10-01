<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import { EditorState } from '@codemirror/state'
  import { EditorView, keymap, highlightActiveLine, drawSelection } from '@codemirror/view'
  import { defaultKeymap, history, historyKeymap } from '@codemirror/commands'
  import { json } from '@codemirror/lang-json'
  import { oneDark } from '@codemirror/theme-one-dark'
  import { linter, type Diagnostic } from '@codemirror/lint'
  import Ajv from 'ajv'

  export let value = ''
  export let dark = false
  export let schema: string = ''
  export let onChange: (v: string) => void = () => {}
  let container: HTMLDivElement
  let view: EditorView | null = null
  const ajv = new Ajv({ allErrors: true, strict: false })

  function createJsonSchemaLinter(schemaText: string) {
    return linter((view) => {
      const diagnostics: Diagnostic[] = []
      const doc = view.state.doc.toString()
      
      // Basic JSON syntax check
      let parsed: any
      try {
        parsed = JSON.parse(doc)
      } catch (e: any) {
        const match = e.message.match(/position (\d+)/)
        const pos = match ? parseInt(match[1]) : 0
        diagnostics.push({
          from: pos,
          to: Math.min(pos + 1, doc.length),
          severity: 'error',
          message: 'Invalid JSON: ' + e.message
        })
        return diagnostics
      }

      // Schema validation if schema provided
      if (schemaText.trim()) {
        let schemaObj: any
        try {
          schemaObj = JSON.parse(schemaText)
        } catch {
          // Skip schema validation if schema is invalid
          return diagnostics
        }

        const validate = ajv.compile(schemaObj)
        const valid = validate(parsed)
        
        if (!valid && validate.errors) {
          for (const error of validate.errors) {
            const path = error.instancePath || '(root)'
            const message = `${path}: ${error.message}`
            diagnostics.push({
              from: 0,
              to: doc.length,
              severity: 'warning',
              message: message
            })
          }
        }
      }

      return diagnostics
    })
  }

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
    
    // Add linter if schema is provided
    if (schema.trim()) {
      base.push(createJsonSchemaLinter(schema))
    }
    
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

  $: if (view && (dark || schema)) {
    // Update theme if dark changes or schema changes
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

