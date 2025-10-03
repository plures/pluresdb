<script lang="ts">
  import { onMount } from 'svelte'
  import { push as toast } from '../lib/toasts'
  import JsonEditor from './JsonEditor.svelte'
  
  let dark = false
  let rules: Array<{
    id: string
    name: string
    description: string
    enabled: boolean
    conditions: RuleCondition[]
    actions: RuleAction[]
    createdAt: number
    updatedAt: number
    lastExecuted?: number
    executionCount: number
  }> = []
  let selectedRule: any = null
  let showNewRule = false
  let newRuleName = ''
  let newRuleDescription = ''
  let showRuleTester = false
  let testResults: any[] = []
  let testing = false
  
  // Rule builder state
  let newCondition: RuleCondition = {
    type: 'field',
    field: '',
    operator: 'equals',
    value: ''
  }
  let newAction: RuleAction = {
    type: 'set_property',
    field: '',
    value: ''
  }
  
  // Available fields and types
  let availableFields: string[] = []
  let availableTypes: string[] = []
  
  $: dark = (document.documentElement.getAttribute('data-theme') === 'dark')
  
  onMount(() => {
    loadRules()
    loadAvailableFields()
  })
  
  function loadRules() {
    try {
      const saved = localStorage.getItem('pluresdb-rules')
      if (saved) {
        rules = JSON.parse(saved)
      }
    } catch (error) {
      console.error('Error loading rules:', error)
    }
  }
  
  function saveRules() {
    try {
      localStorage.setItem('pluresdb-rules', JSON.stringify(rules))
    } catch (error) {
      console.error('Error saving rules:', error)
    }
  }
  
  async function loadAvailableFields() {
    try {
      const res = await fetch('/api/list')
      const nodes = await res.json()
      
      const fields = new Set<string>()
      const types = new Set<string>()
      
      for (const node of nodes) {
        if (node.data.type) types.add(node.data.type)
        Object.keys(node.data).forEach(field => fields.add(field))
      }
      
      availableFields = Array.from(fields).sort()
      availableTypes = Array.from(types).sort()
    } catch (error) {
      console.error('Error loading fields:', error)
    }
  }
  
  function createRule() {
    if (!newRuleName.trim()) {
      toast('Please enter a rule name', 'error')
      return
    }
    
    const rule = {
      id: `rule-${Date.now()}`,
      name: newRuleName,
      description: newRuleDescription,
      enabled: true,
      conditions: [],
      actions: [],
      createdAt: Date.now(),
      updatedAt: Date.now(),
      executionCount: 0
    }
    
    rules = [...rules, rule]
    selectedRule = rule
    newRuleName = ''
    newRuleDescription = ''
    showNewRule = false
    saveRules()
    toast('Rule created successfully', 'success')
  }
  
  function selectRule(rule: any) {
    selectedRule = rule
  }
  
  function deleteRule(ruleId: string) {
    if (confirm('Are you sure you want to delete this rule?')) {
      rules = rules.filter(r => r.id !== ruleId)
      if (selectedRule?.id === ruleId) {
        selectedRule = null
      }
      saveRules()
      toast('Rule deleted', 'success')
    }
  }
  
  function toggleRule(ruleId: string) {
    const rule = rules.find(r => r.id === ruleId)
    if (rule) {
      rule.enabled = !rule.enabled
      rule.updatedAt = Date.now()
      saveRules()
      toast(`Rule ${rule.enabled ? 'enabled' : 'disabled'}`, 'success')
    }
  }
  
  function addCondition() {
    if (!selectedRule) return
    
    const condition = { ...newCondition }
    selectedRule.conditions = [...selectedRule.conditions, condition]
    selectedRule.updatedAt = Date.now()
    saveRules()
    
    // Reset form
    newCondition = {
      type: 'field',
      field: '',
      operator: 'equals',
      value: ''
    }
  }
  
  function removeCondition(index: number) {
    if (!selectedRule) return
    
    selectedRule.conditions = selectedRule.conditions.filter((_: any, i: number) => i !== index)
    selectedRule.updatedAt = Date.now()
    saveRules()
  }
  
  function addAction() {
    if (!selectedRule) return
    
    const action = { ...newAction }
    selectedRule.actions = [...selectedRule.actions, action]
    selectedRule.updatedAt = Date.now()
    saveRules()
    
    // Reset form
    newAction = {
      type: 'set_property',
      field: '',
      value: ''
    }
  }
  
  function removeAction(index: number) {
    if (!selectedRule) return
    
    selectedRule.actions = selectedRule.actions.filter((_: any, i: number) => i !== index)
    selectedRule.updatedAt = Date.now()
    saveRules()
  }
  
  async function testRule() {
    if (!selectedRule) return
    
    testing = true
    try {
      const res = await fetch('/api/test-rule', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          conditions: selectedRule.conditions,
          actions: selectedRule.actions
        })
      })
      
      if (!res.ok) throw new Error('Rule testing failed')
      
      testResults = await res.json()
      showRuleTester = true
      toast(`Rule test completed (${testResults.length} matches)`, 'success')
    } catch (error) {
      toast('Rule testing failed', 'error')
      console.error('Rule testing error:', error)
    } finally {
      testing = false
    }
  }
  
  async function executeRule() {
    if (!selectedRule) return
    
    try {
      const res = await fetch('/api/execute-rule', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          ruleId: selectedRule.id,
          conditions: selectedRule.conditions,
          actions: selectedRule.actions
        })
      })
      
      if (!res.ok) throw new Error('Rule execution failed')
      
      const result = await res.json()
      selectedRule.executionCount++
      selectedRule.lastExecuted = Date.now()
      selectedRule.updatedAt = Date.now()
      saveRules()
      
      toast(`Rule executed successfully (${result.affected} nodes)`, 'success')
    } catch (error) {
      toast('Rule execution failed', 'error')
      console.error('Rule execution error:', error)
    }
  }
  
  function exportRule(rule: any) {
    const data = {
      name: rule.name,
      description: rule.description,
      conditions: rule.conditions,
      actions: rule.actions,
      exportedAt: new Date().toISOString()
    }
    
    const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `${rule.name.replace(/[^a-z0-9]/gi, '_').toLowerCase()}.json`
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
    URL.revokeObjectURL(url)
    
    toast('Rule exported', 'success')
  }
  
  function formatTimestamp(timestamp: number): string {
    return new Date(timestamp).toLocaleString()
  }
</script>

<section aria-labelledby="rules-builder-heading">
  <h3 id="rules-builder-heading">Rules Builder</h3>
  
  <div class="rules-layout">
    <!-- Rules List -->
    <div class="rules-sidebar">
      <div class="sidebar-header">
        <h4>Rules ({rules.length})</h4>
        <button on:click={() => showNewRule = true} class="primary">
          New Rule
        </button>
      </div>
      
      <div class="rules-list">
        {#each rules as rule}
          <div 
            class="rule-item"
            class:selected={selectedRule?.id === rule.id}
            class:disabled={!rule.enabled}
            role="button"
            tabindex="0"
            on:click={() => selectRule(rule)}
            on:keydown={(e) => e.key === 'Enter' && selectRule(rule)}
          >
            <div class="rule-header">
              <span class="rule-name">{rule.name}</span>
              <div class="rule-actions">
                <button 
                  on:click|stopPropagation={() => toggleRule(rule.id)}
                  class="small"
                  title={rule.enabled ? 'Disable rule' : 'Enable rule'}
                >
                  {rule.enabled ? '✅' : '❌'}
                </button>
                <button 
                  on:click|stopPropagation={() => exportRule(rule)}
                  class="small"
                  title="Export rule"
                >
                  📤
                </button>
                <button 
                  on:click|stopPropagation={() => deleteRule(rule.id)}
                  class="small"
                  title="Delete rule"
                >
                  🗑️
                </button>
              </div>
            </div>
            <div class="rule-meta">
              <span class="execution-count">{rule.executionCount} executions</span>
              <span class="updated-time">
                {formatTimestamp(rule.updatedAt)}
              </span>
            </div>
            {#if rule.description}
              <div class="rule-description">
                {rule.description}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    </div>
    
    <!-- Rule Builder -->
    <div class="rule-builder">
      {#if selectedRule}
        <div class="builder-header">
          <h4>{selectedRule.name}</h4>
          <div class="builder-actions">
            <button on:click={testRule} disabled={testing} class="secondary">
              {testing ? 'Testing...' : 'Test Rule'}
            </button>
            <button on:click={executeRule} class="primary">
              Execute Rule
            </button>
          </div>
        </div>
        
        <div class="rule-content">
          <!-- Conditions Section -->
          <div class="conditions-section">
            <h5>Conditions ({selectedRule.conditions.length})</h5>
            <div class="conditions-list">
              {#each selectedRule.conditions as condition, index}
                <div class="condition-item">
                  <div class="condition-content">
                    <select bind:value={condition.field} on:change={() => {}}>
                      {#each availableFields as field}
                        <option value={field}>{field}</option>
                      {/each}
                    </select>
                    <select bind:value={condition.operator} on:change={() => {}}>
                      <option value="equals">equals</option>
                      <option value="not_equals">not equals</option>
                      <option value="contains">contains</option>
                      <option value="not_contains">not contains</option>
                      <option value="starts_with">starts with</option>
                      <option value="ends_with">ends with</option>
                      <option value="greater_than">greater than</option>
                      <option value="less_than">less than</option>
                      <option value="is_empty">is empty</option>
                      <option value="is_not_empty">is not empty</option>
                    </select>
                    <input 
                      type="text" 
                      bind:value={condition.value} 
                      placeholder="Value..."
                    />
                  </div>
                  <button on:click={() => removeCondition(index)} class="small remove">
                    Remove
                  </button>
                </div>
              {/each}
            </div>
            
            <div class="add-condition">
              <h6>Add New Condition</h6>
              <div class="condition-form">
                <select bind:value={newCondition.field}>
                  {#each availableFields as field}
                    <option value={field}>{field}</option>
                  {/each}
                </select>
                <select bind:value={newCondition.operator}>
                  <option value="equals">equals</option>
                  <option value="not_equals">not equals</option>
                  <option value="contains">contains</option>
                  <option value="not_contains">not contains</option>
                  <option value="starts_with">starts with</option>
                  <option value="ends_with">ends with</option>
                  <option value="greater_than">greater than</option>
                  <option value="less_than">less than</option>
                  <option value="is_empty">is empty</option>
                  <option value="is_not_empty">is not empty</option>
                </select>
                <input 
                  type="text" 
                  bind:value={newCondition.value} 
                  placeholder="Value..."
                />
                <button on:click={addCondition} class="small">
                  Add Condition
                </button>
              </div>
            </div>
          </div>
          
          <!-- Actions Section -->
          <div class="actions-section">
            <h5>Actions ({selectedRule.actions.length})</h5>
            <div class="actions-list">
              {#each selectedRule.actions as action, index}
                <div class="action-item">
                  <div class="action-content">
                    <select bind:value={action.type} on:change={() => {}}>
                      <option value="set_property">Set Property</option>
                      <option value="create_relation">Create Relation</option>
                      <option value="delete_property">Delete Property</option>
                      <option value="add_tag">Add Tag</option>
                      <option value="remove_tag">Remove Tag</option>
                    </select>
                    <input 
                      type="text" 
                      bind:value={action.field} 
                      placeholder="Field name..."
                    />
                    <input 
                      type="text" 
                      bind:value={action.value} 
                      placeholder="Value..."
                    />
                  </div>
                  <button on:click={() => removeAction(index)} class="small remove">
                    Remove
                  </button>
                </div>
              {/each}
            </div>
            
            <div class="add-action">
              <h6>Add New Action</h6>
              <div class="action-form">
                <select bind:value={newAction.type}>
                  <option value="set_property">Set Property</option>
                  <option value="create_relation">Create Relation</option>
                  <option value="delete_property">Delete Property</option>
                  <option value="add_tag">Add Tag</option>
                  <option value="remove_tag">Remove Tag</option>
                </select>
                <input 
                  type="text" 
                  bind:value={newAction.field} 
                  placeholder="Field name..."
                />
                <input 
                  type="text" 
                  bind:value={newAction.value} 
                  placeholder="Value..."
                />
                <button on:click={addAction} class="small">
                  Add Action
                </button>
              </div>
            </div>
          </div>
        </div>
      {:else}
        <div class="no-rule">
          <p>Select a rule or create a new one to get started</p>
        </div>
      {/if}
    </div>
  </div>
  
  <!-- New Rule Dialog -->
  {#if showNewRule}
    <div class="dialog-overlay">
      <div class="dialog">
        <h4>Create New Rule</h4>
        <input 
          type="text" 
          bind:value={newRuleName} 
          placeholder="Enter rule name..."
          on:keydown={(e) => e.key === 'Enter' && createRule()}
        />
        <textarea 
          bind:value={newRuleDescription} 
          placeholder="Enter rule description (optional)..."
          rows="3"
        ></textarea>
        <div class="dialog-actions">
          <button on:click={createRule} disabled={!newRuleName.trim()}>
            Create
          </button>
          <button on:click={() => showNewRule = false} class="secondary">
            Cancel
          </button>
        </div>
      </div>
    </div>
  {/if}
  
  <!-- Rule Tester Dialog -->
  {#if showRuleTester}
    <div class="dialog-overlay">
      <div class="dialog large">
        <h4>Rule Test Results ({testResults.length} matches)</h4>
        <div class="test-results">
          {#each testResults as result, index}
            <div class="test-result-item">
              <div class="result-header">
                <span class="result-id">{result.id}</span>
                <span class="result-type">{result.data.type || 'unknown'}</span>
              </div>
              <div class="result-preview">
                {JSON.stringify(result.data).substring(0, 100)}...
              </div>
            </div>
          {/each}
        </div>
        <div class="dialog-actions">
          <button on:click={() => showRuleTester = false} class="secondary">
            Close
          </button>
        </div>
      </div>
    </div>
  {/if}
</section>

<style>
  .rules-layout {
    display: grid;
    grid-template-columns: 300px 1fr;
    gap: 1rem;
    height: 600px;
  }
  
  .rules-sidebar {
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    padding: 1rem;
    overflow-y: auto;
    background: var(--pico-muted-border-color);
  }
  
  .sidebar-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }
  
  .rules-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  
  .rule-item {
    padding: 0.75rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 4px;
    background: var(--pico-background-color);
    cursor: pointer;
    transition: all 0.2s;
  }
  
  .rule-item:hover {
    background: var(--pico-muted-border-color);
  }
  
  .rule-item.selected {
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
  }
  
  .rule-item.disabled {
    opacity: 0.6;
  }
  
  .rule-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.25rem;
  }
  
  .rule-name {
    font-weight: 600;
    font-size: 0.875rem;
  }
  
  .rule-actions {
    display: flex;
    gap: 0.25rem;
  }
  
  .rule-meta {
    display: flex;
    justify-content: space-between;
    font-size: 0.75rem;
    opacity: 0.7;
    margin-bottom: 0.25rem;
  }
  
  .rule-description {
    font-size: 0.75rem;
    opacity: 0.8;
    font-style: italic;
  }
  
  .rule-builder {
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    padding: 1rem;
    overflow-y: auto;
    background: var(--pico-background-color);
  }
  
  .builder-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
    padding-bottom: 1rem;
    border-bottom: 1px solid var(--pico-muted-border-color);
  }
  
  .builder-actions {
    display: flex;
    gap: 0.5rem;
  }
  
  .rule-content {
    display: flex;
    flex-direction: column;
    gap: 2rem;
  }
  
  .conditions-section, .actions-section {
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    padding: 1rem;
    background: var(--pico-muted-border-color);
  }
  
  .conditions-list, .actions-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    margin-bottom: 1rem;
  }
  
  .condition-item, .action-item {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    padding: 0.5rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 4px;
    background: var(--pico-background-color);
  }
  
  .condition-content, .action-content {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    flex: 1;
  }
  
  .condition-content select,
  .condition-content input,
  .action-content select,
  .action-content input {
    flex: 1;
  }
  
  .add-condition, .add-action {
    border-top: 1px solid var(--pico-muted-border-color);
    padding-top: 1rem;
  }
  
  .condition-form, .action-form {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }
  
  .condition-form select,
  .condition-form input,
  .action-form select,
  .action-form input {
    flex: 1;
  }
  
  .test-results {
    max-height: 400px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  
  .test-result-item {
    padding: 0.75rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 4px;
    background: var(--pico-muted-border-color);
  }
  
  .result-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.25rem;
  }
  
  .result-id {
    font-weight: 600;
    font-size: 0.875rem;
  }
  
  .result-type {
    font-size: 0.75rem;
    opacity: 0.8;
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
    padding: 0.125rem 0.375rem;
    border-radius: 3px;
  }
  
  .result-preview {
    font-size: 0.75rem;
    opacity: 0.7;
    font-family: monospace;
  }
  
  .no-rule {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--pico-muted-color);
    font-style: italic;
  }
  
  .dialog-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }
  
  .dialog {
    background: var(--pico-background-color);
    padding: 2rem;
    border-radius: 8px;
    min-width: 400px;
    max-width: 500px;
  }
  
  .dialog.large {
    min-width: 600px;
    max-width: 800px;
  }
  
  .dialog h4 {
    margin-bottom: 1rem;
  }
  
  .dialog input,
  .dialog textarea {
    width: 100%;
    margin-bottom: 1rem;
  }
  
  .dialog-actions {
    display: flex;
    gap: 0.5rem;
    justify-content: flex-end;
  }
  
  .small {
    padding: 0.25rem 0.5rem;
    font-size: 0.75rem;
  }
  
  .remove {
    background: var(--error-color);
    color: white;
    border: none;
  }
  
  @media (max-width: 768px) {
    .rules-layout {
      grid-template-columns: 1fr;
      height: auto;
    }
    
    .rules-sidebar {
      max-height: 200px;
    }
    
    .condition-form,
    .action-form {
      flex-direction: column;
    }
  }
</style>
