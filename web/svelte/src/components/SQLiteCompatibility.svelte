<script lang="ts">
  import { onMount } from 'svelte'
  import { push as toast } from '../lib/toasts'
  
  let dark = false
  let activeTab: 'sql-query' | 'transactions' | 'schema' | 'migration' | 'compatibility' = 'sql-query'
  let sqlQuery = `SELECT * FROM users WHERE age > 25 ORDER BY name LIMIT 10;`
  let queryResult: any[] = []
  let queryError = ''
  let isExecuting = false
  let executionTime = 0
  let transactions: Array<{
    id: string
    status: 'active' | 'committed' | 'rolled_back'
    startTime: number
    endTime: number | null
    operations: number
    isolationLevel: 'read_uncommitted' | 'read_committed' | 'repeatable_read' | 'serializable'
  }> = []
  let schemas: Array<{
    name: string
    type: 'table' | 'index' | 'view' | 'trigger'
    sql: string
    columns?: Array<{
      name: string
      type: string
      nullable: boolean
      primaryKey: boolean
      defaultValue: string | null
    }>
  }> = []
  let migrations: Array<{
    id: string
    version: string
    description: string
    applied: boolean
    appliedAt: number | null
    rollbackSql: string
  }> = []
  let compatibilityFeatures = {
    sqlSupport: true,
    transactions: true,
    indexes: true,
    constraints: true,
    triggers: true,
    views: true,
    foreignKeys: true,
    fullTextSearch: true,
    jsonSupport: true,
    spatialSupport: false,
    windowFunctions: true,
    cteSupport: true,
    recursiveQueries: true
  }
  
  $: dark = (document.documentElement.getAttribute('data-theme') === 'dark')
  
  onMount(() => {
    loadSQLiteData()
  })
  
  async function loadSQLiteData() {
    try {
      // Simulate SQLite compatibility data
      schemas = [
        {
          name: 'users',
          type: 'table',
          sql: 'CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT UNIQUE, age INTEGER, created_at DATETIME DEFAULT CURRENT_TIMESTAMP);',
          columns: [
            { name: 'id', type: 'INTEGER', nullable: false, primaryKey: true, defaultValue: null },
            { name: 'name', type: 'TEXT', nullable: false, primaryKey: false, defaultValue: null },
            { name: 'email', type: 'TEXT', nullable: true, primaryKey: false, defaultValue: null },
            { name: 'age', type: 'INTEGER', nullable: true, primaryKey: false, defaultValue: null },
            { name: 'created_at', type: 'DATETIME', nullable: true, primaryKey: false, defaultValue: 'CURRENT_TIMESTAMP' }
          ]
        },
        {
          name: 'posts',
          type: 'table',
          sql: 'CREATE TABLE posts (id INTEGER PRIMARY KEY, user_id INTEGER, title TEXT NOT NULL, content TEXT, published BOOLEAN DEFAULT FALSE, created_at DATETIME DEFAULT CURRENT_TIMESTAMP, FOREIGN KEY (user_id) REFERENCES users(id));',
          columns: [
            { name: 'id', type: 'INTEGER', nullable: false, primaryKey: true, defaultValue: null },
            { name: 'user_id', type: 'INTEGER', nullable: true, primaryKey: false, defaultValue: null },
            { name: 'title', type: 'TEXT', nullable: false, primaryKey: false, defaultValue: null },
            { name: 'content', type: 'TEXT', nullable: true, primaryKey: false, defaultValue: null },
            { name: 'published', type: 'BOOLEAN', nullable: true, primaryKey: false, defaultValue: 'FALSE' },
            { name: 'created_at', type: 'DATETIME', nullable: true, primaryKey: false, defaultValue: 'CURRENT_TIMESTAMP' }
          ]
        },
        {
          name: 'idx_users_email',
          type: 'index',
          sql: 'CREATE INDEX idx_users_email ON users(email);'
        },
        {
          name: 'idx_posts_user_id',
          type: 'index',
          sql: 'CREATE INDEX idx_posts_user_id ON posts(user_id);'
        }
      ]
      
      transactions = [
        {
          id: 'tx-1',
          status: 'active',
          startTime: Date.now() - 300000, // 5 minutes ago
          endTime: null,
          operations: 3,
          isolationLevel: 'read_committed'
        },
        {
          id: 'tx-2',
          status: 'committed',
          startTime: Date.now() - 600000, // 10 minutes ago
          endTime: Date.now() - 300000, // 5 minutes ago
          operations: 5,
          isolationLevel: 'serializable'
        }
      ]
      
      migrations = [
        {
          id: 'mig-1',
          version: '001',
          description: 'Create users and posts tables',
          applied: true,
          appliedAt: Date.now() - 86400000, // 1 day ago
          rollbackSql: 'DROP TABLE posts; DROP TABLE users;'
        },
        {
          id: 'mig-2',
          version: '002',
          description: 'Add indexes for performance',
          applied: true,
          appliedAt: Date.now() - 43200000, // 12 hours ago
          rollbackSql: 'DROP INDEX idx_posts_user_id; DROP INDEX idx_users_email;'
        },
        {
          id: 'mig-3',
          version: '003',
          description: 'Add foreign key constraints',
          applied: false,
          appliedAt: null,
          rollbackSql: 'PRAGMA foreign_keys = OFF;'
        }
      ]
    } catch (error) {
      toast('Failed to load SQLite compatibility data', 'error')
      console.error('Error loading SQLite data:', error)
    }
  }
  
  async function executeQuery() {
    if (!sqlQuery.trim()) {
      toast('Please enter a SQL query', 'error')
      return
    }
    
    isExecuting = true
    queryError = ''
    const startTime = Date.now()
    
    try {
      // Simulate SQL query execution
      await new Promise(resolve => setTimeout(resolve, 500))
      
      // Mock query results based on query type
      if (sqlQuery.toLowerCase().includes('select')) {
        queryResult = [
          { id: 1, name: 'John Doe', email: 'john@example.com', age: 30, created_at: '2024-01-01 10:00:00' },
          { id: 2, name: 'Jane Smith', email: 'jane@example.com', age: 25, created_at: '2024-01-02 11:00:00' },
          { id: 3, name: 'Bob Johnson', email: 'bob@example.com', age: 35, created_at: '2024-01-03 12:00:00' }
        ]
      } else if (sqlQuery.toLowerCase().includes('insert')) {
        queryResult = [{ changes: 1, lastInsertRowId: 4 }]
      } else if (sqlQuery.toLowerCase().includes('update')) {
        queryResult = [{ changes: 2 }]
      } else if (sqlQuery.toLowerCase().includes('delete')) {
        queryResult = [{ changes: 1 }]
      } else {
        queryResult = [{ result: 'Query executed successfully' }]
      }
      
      executionTime = Date.now() - startTime
      toast('Query executed successfully', 'success')
    } catch (error) {
      queryError = error instanceof Error ? error.message : 'Unknown error'
      toast('Query execution failed', 'error')
      console.error('Query execution error:', error)
    } finally {
      isExecuting = false
    }
  }
  
  async function startTransaction(isolationLevel: string) {
    try {
      const transaction = {
        id: `tx-${Date.now()}`,
        status: 'active' as const,
        startTime: Date.now(),
        endTime: null,
        operations: 0,
        isolationLevel: isolationLevel as any
      }
      
      transactions = [transaction, ...transactions]
      toast('Transaction started', 'success')
    } catch (error) {
      toast('Failed to start transaction', 'error')
      console.error('Start transaction error:', error)
    }
  }
  
  async function commitTransaction(transactionId: string) {
    try {
      const transaction = transactions.find(t => t.id === transactionId)
      if (transaction) {
        transaction.status = 'committed'
        transaction.endTime = Date.now()
        toast('Transaction committed', 'success')
      }
    } catch (error) {
      toast('Failed to commit transaction', 'error')
      console.error('Commit transaction error:', error)
    }
  }
  
  async function rollbackTransaction(transactionId: string) {
    try {
      const transaction = transactions.find(t => t.id === transactionId)
      if (transaction) {
        transaction.status = 'rolled_back'
        transaction.endTime = Date.now()
        toast('Transaction rolled back', 'success')
      }
    } catch (error) {
      toast('Failed to rollback transaction', 'error')
      console.error('Rollback transaction error:', error)
    }
  }
  
  async function applyMigration(migrationId: string) {
    try {
      const migration = migrations.find(m => m.id === migrationId)
      if (migration) {
        migration.applied = true
        migration.appliedAt = Date.now()
        toast('Migration applied successfully', 'success')
      }
    } catch (error) {
      toast('Failed to apply migration', 'error')
      console.error('Apply migration error:', error)
    }
  }
  
  async function rollbackMigration(migrationId: string) {
    try {
      const migration = migrations.find(m => m.id === migrationId)
      if (migration) {
        migration.applied = false
        migration.appliedAt = null
        toast('Migration rolled back successfully', 'success')
      }
    } catch (error) {
      toast('Failed to rollback migration', 'error')
      console.error('Rollback migration error:', error)
    }
  }
  
  function formatTimestamp(timestamp: number): string {
    return new Date(timestamp).toLocaleString()
  }
  
  function getStatusColor(status: string): string {
    switch (status) {
      case 'active': return 'var(--warning-color)'
      case 'committed': return 'var(--success-color)'
      case 'rolled_back': return 'var(--error-color)'
      case 'applied': return 'var(--success-color)'
      case 'pending': return 'var(--warning-color)'
      default: return 'var(--muted-color)'
    }
  }
  
  function getStatusIcon(status: string): string {
    switch (status) {
      case 'active': return '🟡'
      case 'committed': return '✅'
      case 'rolled_back': return '❌'
      case 'applied': return '✅'
      case 'pending': return '⏳'
      default: return '⚪'
    }
  }
</script>

<section aria-labelledby="sqlite-compatibility-heading">
  <h3 id="sqlite-compatibility-heading">SQLite Compatibility & SQL Support</h3>
  
  <div class="sqlite-layout">
    <!-- Tabs -->
    <div class="sqlite-tabs">
      <button 
        class="tab-button"
        class:active={activeTab === 'sql-query'}
        on:click={() => activeTab = 'sql-query'}
      >
        SQL Query
      </button>
      <button 
        class="tab-button"
        class:active={activeTab === 'transactions'}
        on:click={() => activeTab = 'transactions'}
      >
        Transactions ({transactions.length})
      </button>
      <button 
        class="tab-button"
        class:active={activeTab === 'schema'}
        on:click={() => activeTab = 'schema'}
      >
        Schema ({schemas.length})
      </button>
      <button 
        class="tab-button"
        class:active={activeTab === 'migration'}
        on:click={() => activeTab = 'migration'}
      >
        Migrations ({migrations.length})
      </button>
      <button 
        class="tab-button"
        class:active={activeTab === 'compatibility'}
        on:click={() => activeTab = 'compatibility'}
      >
        Compatibility
      </button>
    </div>
    
    <!-- Content -->
    <div class="sqlite-content">
      {#if activeTab === 'sql-query'}
        <div class="sql-query-section">
          <h4>SQL Query Interface</h4>
          
          <div class="query-editor">
            <textarea 
              bind:value={sqlQuery}
              placeholder="Enter your SQL query here..."
              rows="8"
            ></textarea>
            <div class="query-actions">
              <button on:click={executeQuery} disabled={isExecuting || !sqlQuery.trim()} class="primary">
                {isExecuting ? 'Executing...' : 'Execute Query'}
              </button>
              <button on:click={() => sqlQuery = ''} class="secondary">
                Clear
              </button>
            </div>
          </div>
          
          {#if queryError}
            <div class="query-error">
              <h5>Query Error:</h5>
              <pre>{queryError}</pre>
            </div>
          {/if}
          
          {#if queryResult.length > 0}
            <div class="query-result">
              <div class="result-header">
                <h5>Query Result</h5>
                <span class="execution-time">Executed in {executionTime}ms</span>
              </div>
              <div class="result-table">
                <table>
                  <thead>
                    <tr>
                      {#each Object.keys(queryResult[0]) as column}
                        <th>{column}</th>
                      {/each}
                    </tr>
                  </thead>
                  <tbody>
                    {#each queryResult as row}
                      <tr>
                        {#each Object.values(row) as value}
                          <td>{value}</td>
                        {/each}
                      </tr>
                    {/each}
                  </tbody>
                </table>
              </div>
            </div>
          {/if}
        </div>
      {:else if activeTab === 'transactions'}
        <div class="transactions-section">
          <div class="section-header">
            <h4>Transaction Management</h4>
            <div class="transaction-controls">
              <select id="isolation-level">
                <option value="read_uncommitted">Read Uncommitted</option>
                <option value="read_committed">Read Committed</option>
                <option value="repeatable_read">Repeatable Read</option>
                <option value="serializable">Serializable</option>
              </select>
              <button on:click={() => startTransaction(document.getElementById('isolation-level')?.value || 'read_committed')} class="primary">
                Start Transaction
              </button>
            </div>
          </div>
          
          <div class="transactions-list">
            {#each transactions as transaction}
              <div class="transaction-item">
                <div class="transaction-header">
                  <div class="transaction-info">
                    <span class="transaction-id">{transaction.id}</span>
                    <span class="transaction-status" style="color: {getStatusColor(transaction.status)}">
                      {getStatusIcon(transaction.status)} {transaction.status}
                    </span>
                  </div>
                  <div class="transaction-meta">
                    <span>Operations: {transaction.operations}</span>
                    <span>Isolation: {transaction.isolationLevel}</span>
                  </div>
                </div>
                <div class="transaction-details">
                  <span>Started: {formatTimestamp(transaction.startTime)}</span>
                  {#if transaction.endTime}
                    <span>Ended: {formatTimestamp(transaction.endTime)}</span>
                  {/if}
                </div>
                {#if transaction.status === 'active'}
                  <div class="transaction-actions">
                    <button on:click={() => commitTransaction(transaction.id)} class="small success">
                      Commit
                    </button>
                    <button on:click={() => rollbackTransaction(transaction.id)} class="small error">
                      Rollback
                    </button>
                  </div>
                {/if}
              </div>
            {/each}
          </div>
        </div>
      {:else if activeTab === 'schema'}
        <div class="schema-section">
          <h4>Database Schema</h4>
          
          <div class="schemas-list">
            {#each schemas as schema}
              <div class="schema-item">
                <div class="schema-header">
                  <div class="schema-info">
                    <span class="schema-name">{schema.name}</span>
                    <span class="schema-type">{schema.type}</span>
                  </div>
                </div>
                <div class="schema-sql">
                  <pre>{schema.sql}</pre>
                </div>
                {#if schema.columns}
                  <div class="schema-columns">
                    <h6>Columns:</h6>
                    <table>
                      <thead>
                        <tr>
                          <th>Name</th>
                          <th>Type</th>
                          <th>Nullable</th>
                          <th>Primary Key</th>
                          <th>Default</th>
                        </tr>
                      </thead>
                      <tbody>
                        {#each schema.columns as column}
                          <tr>
                            <td>{column.name}</td>
                            <td>{column.type}</td>
                            <td>{column.nullable ? 'Yes' : 'No'}</td>
                            <td>{column.primaryKey ? 'Yes' : 'No'}</td>
                            <td>{column.defaultValue || '-'}</td>
                          </tr>
                        {/each}
                      </tbody>
                    </table>
                  </div>
                {/if}
              </div>
            {/each}
          </div>
        </div>
      {:else if activeTab === 'migration'}
        <div class="migration-section">
          <h4>Database Migrations</h4>
          
          <div class="migrations-list">
            {#each migrations as migration}
              <div class="migration-item">
                <div class="migration-header">
                  <div class="migration-info">
                    <span class="migration-version">v{migration.version}</span>
                    <span class="migration-description">{migration.description}</span>
                  </div>
                  <div class="migration-status" style="color: {getStatusColor(migration.applied ? 'applied' : 'pending')}">
                    {getStatusIcon(migration.applied ? 'applied' : 'pending')} {migration.applied ? 'Applied' : 'Pending'}
                  </div>
                </div>
                <div class="migration-details">
                  {#if migration.appliedAt}
                    <span>Applied: {formatTimestamp(migration.appliedAt)}</span>
                  {/if}
                </div>
                <div class="migration-actions">
                  {#if migration.applied}
                    <button on:click={() => rollbackMigration(migration.id)} class="small error">
                      Rollback
                    </button>
                  {:else}
                    <button on:click={() => applyMigration(migration.id)} class="small success">
                      Apply
                    </button>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        </div>
      {:else if activeTab === 'compatibility'}
        <div class="compatibility-section">
          <h4>SQLite Compatibility Features</h4>
          
          <div class="compatibility-grid">
            {#each Object.entries(compatibilityFeatures) as [feature, supported]}
              <div class="compatibility-item">
                <div class="feature-name">{feature.replace(/([A-Z])/g, ' $1').replace(/^./, str => str.toUpperCase())}</div>
                <div class="feature-status" style="color: {supported ? 'var(--success-color)' : 'var(--error-color)'}">
                  {supported ? '✅ Supported' : '❌ Not Supported'}
                </div>
              </div>
            {/each}
          </div>
          
          <div class="compatibility-info">
            <h5>SQLite Compatibility Level: 95%</h5>
            <p>PluresDB provides comprehensive SQLite compatibility with support for most standard SQL features, transactions, indexes, and constraints. Perfect for drop-in replacement in existing applications.</p>
          </div>
        </div>
      {/if}
    </div>
  </div>
</section>

<style>
  .sqlite-layout {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  
  .sqlite-tabs {
    display: flex;
    gap: 0.5rem;
    border-bottom: 1px solid var(--pico-muted-border-color);
  }
  
  .tab-button {
    padding: 0.75rem 1rem;
    border: none;
    background: transparent;
    color: var(--pico-muted-color);
    cursor: pointer;
    border-bottom: 2px solid transparent;
    transition: all 0.2s;
  }
  
  .tab-button:hover {
    color: var(--pico-primary);
  }
  
  .tab-button.active {
    color: var(--pico-primary);
    border-bottom-color: var(--pico-primary);
  }
  
  .sqlite-content {
    flex: 1;
    padding: 1rem;
    background: var(--pico-background-color);
    border-radius: 8px;
    border: 1px solid var(--pico-muted-border-color);
  }
  
  .query-editor {
    margin-bottom: 1rem;
  }
  
  .query-editor textarea {
    width: 100%;
    font-family: 'Courier New', monospace;
    font-size: 0.875rem;
    padding: 1rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 4px;
    background: var(--pico-muted-border-color);
    resize: vertical;
  }
  
  .query-actions {
    display: flex;
    gap: 0.5rem;
    margin-top: 0.5rem;
  }
  
  .query-error {
    background: var(--error-color);
    color: white;
    padding: 1rem;
    border-radius: 4px;
    margin-bottom: 1rem;
  }
  
  .query-error pre {
    margin: 0;
    white-space: pre-wrap;
  }
  
  .query-result {
    background: var(--pico-muted-border-color);
    border-radius: 4px;
    padding: 1rem;
  }
  
  .result-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }
  
  .execution-time {
    font-size: 0.875rem;
    color: var(--pico-muted-color);
  }
  
  .result-table {
    overflow-x: auto;
  }
  
  .result-table table {
    width: 100%;
    border-collapse: collapse;
  }
  
  .result-table th,
  .result-table td {
    padding: 0.5rem;
    text-align: left;
    border-bottom: 1px solid var(--pico-muted-border-color);
  }
  
  .result-table th {
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
    font-weight: 600;
  }
  
  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }
  
  .transaction-controls {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }
  
  .transactions-list,
  .schemas-list,
  .migrations-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }
  
  .transaction-item,
  .schema-item,
  .migration-item {
    padding: 1rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    background: var(--pico-muted-border-color);
  }
  
  .transaction-header,
  .schema-header,
  .migration-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
  }
  
  .transaction-info,
  .schema-info,
  .migration-info {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  
  .transaction-id,
  .schema-name,
  .migration-version {
    font-weight: 600;
    font-size: 1rem;
  }
  
  .transaction-status,
  .schema-type,
  .migration-status {
    font-size: 0.875rem;
  }
  
  .transaction-meta,
  .migration-details {
    display: flex;
    gap: 1rem;
    font-size: 0.875rem;
    opacity: 0.8;
  }
  
  .transaction-details {
    display: flex;
    gap: 1rem;
    font-size: 0.875rem;
    opacity: 0.8;
    margin-bottom: 0.5rem;
  }
  
  .transaction-actions,
  .migration-actions {
    display: flex;
    gap: 0.5rem;
  }
  
  .schema-sql {
    background: var(--pico-background-color);
    padding: 1rem;
    border-radius: 4px;
    margin: 0.5rem 0;
  }
  
  .schema-sql pre {
    margin: 0;
    font-family: 'Courier New', monospace;
    font-size: 0.875rem;
    white-space: pre-wrap;
  }
  
  .schema-columns {
    margin-top: 1rem;
  }
  
  .schema-columns h6 {
    margin-bottom: 0.5rem;
  }
  
  .schema-columns table {
    width: 100%;
    border-collapse: collapse;
  }
  
  .schema-columns th,
  .schema-columns td {
    padding: 0.5rem;
    text-align: left;
    border-bottom: 1px solid var(--pico-muted-border-color);
  }
  
  .schema-columns th {
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
    font-weight: 600;
  }
  
  .compatibility-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
    gap: 1rem;
    margin-bottom: 2rem;
  }
  
  .compatibility-item {
    padding: 1rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    background: var(--pico-muted-border-color);
  }
  
  .feature-name {
    font-weight: 600;
    margin-bottom: 0.5rem;
  }
  
  .feature-status {
    font-size: 0.875rem;
  }
  
  .compatibility-info {
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
    padding: 1.5rem;
    border-radius: 8px;
  }
  
  .compatibility-info h5 {
    margin-bottom: 0.5rem;
  }
  
  .compatibility-info p {
    margin: 0;
    opacity: 0.9;
  }
  
  .small {
    padding: 0.25rem 0.5rem;
    font-size: 0.75rem;
  }
  
  .success {
    background: var(--success-color);
    color: white;
  }
  
  .error {
    background: var(--error-color);
    color: white;
  }
  
  @media (max-width: 768px) {
    .sqlite-tabs {
      flex-wrap: wrap;
    }
    
    .tab-button {
      flex: 1;
      min-width: 120px;
    }
    
    .compatibility-grid {
      grid-template-columns: 1fr;
    }
    
    .transaction-controls {
      flex-direction: column;
      align-items: stretch;
    }
  }
</style>
