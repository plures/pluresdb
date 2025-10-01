<script lang="ts">
  import { onMount } from 'svelte'
  import { push as toast } from '../lib/toasts'
  
  let dark = false
  let activeTab: 'users' | 'roles' | 'policies' | 'tokens' | 'settings' = 'users'
  let users: Array<{
    id: string
    username: string
    email: string
    role: string
    status: 'active' | 'inactive' | 'suspended'
    lastLogin: number
    createdAt: number
    permissions: string[]
  }> = []
  let roles: Array<{
    id: string
    name: string
    description: string
    permissions: string[]
    userCount: number
    createdAt: number
  }> = []
  let policies: Array<{
    id: string
    name: string
    description: string
    resource: string
    actions: string[]
    conditions: string[]
    effect: 'allow' | 'deny'
    priority: number
    createdAt: number
  }> = []
  let tokens: Array<{
    id: string
    name: string
    token: string
    permissions: string[]
    expiresAt: number
    lastUsed: number
    status: 'active' | 'expired' | 'revoked'
    createdAt: number
  }> = []
  let selectedUser: any = null
  let selectedRole: any = null
  let selectedPolicy: any = null
  let selectedToken: any = null
  let showUserDialog = false
  let showRoleDialog = false
  let showPolicyDialog = false
  let showTokenDialog = false
  let newUser = { username: '', email: '', role: '', password: '' }
  let newRole = { name: '', description: '', permissions: [] }
  let newPolicy = { name: '', description: '', resource: '', actions: [], conditions: [], effect: 'allow' as const, priority: 0 }
  let newToken = { name: '', permissions: [], expiresIn: 30 }
  
  // Available permissions and resources
  let availablePermissions = [
    'read:data', 'write:data', 'delete:data',
    'read:types', 'write:types', 'delete:types',
    'read:history', 'write:history',
    'read:graph', 'write:graph',
    'read:vector', 'write:vector',
    'read:search', 'write:search',
    'read:notebooks', 'write:notebooks',
    'read:queries', 'write:queries',
    'read:rules', 'write:rules',
    'read:tasks', 'write:tasks',
    'read:mesh', 'write:mesh',
    'read:storage', 'write:storage',
    'read:profiling', 'write:profiling',
    'admin:users', 'admin:roles', 'admin:policies', 'admin:tokens'
  ]
  let availableResources = [
    'data', 'types', 'history', 'graph', 'vector', 'search',
    'notebooks', 'queries', 'rules', 'tasks', 'mesh', 'storage', 'profiling'
  ]
  let availableActions = [
    'read', 'write', 'delete', 'create', 'update', 'execute', 'manage'
  ]
  
  $: dark = (document.documentElement.getAttribute('data-theme') === 'dark')
  
  onMount(() => {
    loadSecurityData()
  })
  
  async function loadSecurityData() {
    try {
      // Simulate security data
      users = [
        {
          id: 'user-1',
          username: 'admin',
          email: 'admin@rusty-gun.local',
          role: 'admin',
          status: 'active',
          lastLogin: Date.now() - 3600000, // 1 hour ago
          createdAt: Date.now() - 86400000, // 1 day ago
          permissions: ['admin:users', 'admin:roles', 'admin:policies', 'admin:tokens']
        },
        {
          id: 'user-2',
          username: 'developer',
          email: 'dev@rusty-gun.local',
          role: 'developer',
          status: 'active',
          lastLogin: Date.now() - 7200000, // 2 hours ago
          createdAt: Date.now() - 172800000, // 2 days ago
          permissions: ['read:data', 'write:data', 'read:types', 'write:types']
        },
        {
          id: 'user-3',
          username: 'viewer',
          email: 'viewer@rusty-gun.local',
          role: 'viewer',
          status: 'active',
          lastLogin: Date.now() - 14400000, // 4 hours ago
          createdAt: Date.now() - 259200000, // 3 days ago
          permissions: ['read:data', 'read:types', 'read:graph']
        }
      ]
      
      roles = [
        {
          id: 'role-1',
          name: 'admin',
          description: 'Full system access',
          permissions: ['admin:users', 'admin:roles', 'admin:policies', 'admin:tokens'],
          userCount: 1,
          createdAt: Date.now() - 86400000
        },
        {
          id: 'role-2',
          name: 'developer',
          description: 'Development and data management',
          permissions: ['read:data', 'write:data', 'delete:data', 'read:types', 'write:types'],
          userCount: 1,
          createdAt: Date.now() - 172800000
        },
        {
          id: 'role-3',
          name: 'viewer',
          description: 'Read-only access',
          permissions: ['read:data', 'read:types', 'read:graph'],
          userCount: 1,
          createdAt: Date.now() - 259200000
        }
      ]
      
      policies = [
        {
          id: 'policy-1',
          name: 'Admin Full Access',
          description: 'Administrators have full access to all resources',
          resource: '*',
          actions: ['*'],
          conditions: ['role:admin'],
          effect: 'allow',
          priority: 100,
          createdAt: Date.now() - 86400000
        },
        {
          id: 'policy-2',
          name: 'Data Access Control',
          description: 'Control data access based on user role',
          resource: 'data',
          actions: ['read', 'write'],
          conditions: ['role:developer', 'role:admin'],
          effect: 'allow',
          priority: 50,
          createdAt: Date.now() - 172800000
        },
        {
          id: 'policy-3',
          name: 'Read-Only Access',
          description: 'Viewers can only read data',
          resource: 'data',
          actions: ['read'],
          conditions: ['role:viewer'],
          effect: 'allow',
          priority: 25,
          createdAt: Date.now() - 259200000
        }
      ]
      
      tokens = [
        {
          id: 'token-1',
          name: 'API Token',
          token: 'rg_****1234',
          permissions: ['read:data', 'write:data'],
          expiresAt: Date.now() + 86400000, // 1 day from now
          lastUsed: Date.now() - 1800000, // 30 minutes ago
          status: 'active',
          createdAt: Date.now() - 3600000 // 1 hour ago
        },
        {
          id: 'token-2',
          name: 'Backup Token',
          token: 'rg_****5678',
          permissions: ['read:data', 'read:storage'],
          expiresAt: Date.now() + 604800000, // 7 days from now
          lastUsed: Date.now() - 86400000, // 1 day ago
          status: 'active',
          createdAt: Date.now() - 172800000 // 2 days ago
        }
      ]
    } catch (error) {
      toast('Failed to load security data', 'error')
      console.error('Error loading security data:', error)
    }
  }
  
  function createUser() {
    if (!newUser.username.trim() || !newUser.email.trim()) {
      toast('Please fill in all required fields', 'error')
      return
    }
    
    const user = {
      id: `user-${Date.now()}`,
      username: newUser.username,
      email: newUser.email,
      role: newUser.role,
      status: 'active' as const,
      lastLogin: 0,
      createdAt: Date.now(),
      permissions: roles.find(r => r.name === newUser.role)?.permissions || []
    }
    
    users = [...users, user]
    newUser = { username: '', email: '', role: '', password: '' }
    showUserDialog = false
    toast('User created successfully', 'success')
  }
  
  function createRole() {
    if (!newRole.name.trim()) {
      toast('Please enter a role name', 'error')
      return
    }
    
    const role = {
      id: `role-${Date.now()}`,
      name: newRole.name,
      description: newRole.description,
      permissions: newRole.permissions,
      userCount: 0,
      createdAt: Date.now()
    }
    
    roles = [...roles, role]
    newRole = { name: '', description: '', permissions: [] }
    showRoleDialog = false
    toast('Role created successfully', 'success')
  }
  
  function createPolicy() {
    if (!newPolicy.name.trim() || !newPolicy.resource.trim()) {
      toast('Please fill in all required fields', 'error')
      return
    }
    
    const policy = {
      id: `policy-${Date.now()}`,
      name: newPolicy.name,
      description: newPolicy.description,
      resource: newPolicy.resource,
      actions: newPolicy.actions,
      conditions: newPolicy.conditions,
      effect: newPolicy.effect,
      priority: newPolicy.priority,
      createdAt: Date.now()
    }
    
    policies = [...policies, policy]
    newPolicy = { name: '', description: '', resource: '', actions: [], conditions: [], effect: 'allow' as const, priority: 0 }
    showPolicyDialog = false
    toast('Policy created successfully', 'success')
  }
  
  function createToken() {
    if (!newToken.name.trim()) {
      toast('Please enter a token name', 'error')
      return
    }
    
    const token = {
      id: `token-${Date.now()}`,
      name: newToken.name,
      token: `rg_${Math.random().toString(36).substr(2, 8)}`,
      permissions: newToken.permissions,
      expiresAt: Date.now() + (newToken.expiresIn * 24 * 60 * 60 * 1000),
      lastUsed: 0,
      status: 'active' as const,
      createdAt: Date.now()
    }
    
    tokens = [...tokens, token]
    newToken = { name: '', permissions: [], expiresIn: 30 }
    showTokenDialog = false
    toast('Token created successfully', 'success')
  }
  
  function deleteUser(userId: string) {
    if (confirm('Are you sure you want to delete this user?')) {
      users = users.filter(u => u.id !== userId)
      if (selectedUser?.id === userId) {
        selectedUser = null
      }
      toast('User deleted', 'success')
    }
  }
  
  function deleteRole(roleId: string) {
    if (confirm('Are you sure you want to delete this role?')) {
      roles = roles.filter(r => r.id !== roleId)
      if (selectedRole?.id === roleId) {
        selectedRole = null
      }
      toast('Role deleted', 'success')
    }
  }
  
  function deletePolicy(policyId: string) {
    if (confirm('Are you sure you want to delete this policy?')) {
      policies = policies.filter(p => p.id !== policyId)
      if (selectedPolicy?.id === policyId) {
        selectedPolicy = null
      }
      toast('Policy deleted', 'success')
    }
  }
  
  function revokeToken(tokenId: string) {
    if (confirm('Are you sure you want to revoke this token?')) {
      const token = tokens.find(t => t.id === tokenId)
      if (token) {
        token.status = 'revoked'
        toast('Token revoked', 'success')
      }
    }
  }
  
  function formatTimestamp(timestamp: number): string {
    return new Date(timestamp).toLocaleString()
  }
  
  function getStatusColor(status: string): string {
    switch (status) {
      case 'active': return 'var(--success-color)'
      case 'inactive': return 'var(--muted-color)'
      case 'suspended': return 'var(--error-color)'
      case 'expired': return 'var(--warning-color)'
      case 'revoked': return 'var(--error-color)'
      default: return 'var(--muted-color)'
    }
  }
  
  function getStatusIcon(status: string): string {
    switch (status) {
      case 'active': return '✅'
      case 'inactive': return '⏸️'
      case 'suspended': return '🚫'
      case 'expired': return '⏰'
      case 'revoked': return '❌'
      default: return '⚪'
    }
  }
  
  function selectUser(user: any) {
    selectedUser = user
  }
  
  function selectRole(role: any) {
    selectedRole = role
  }
  
  function selectPolicy(policy: any) {
    selectedPolicy = policy
  }
  
  function selectToken(token: any) {
    selectedToken = token
  }
</script>

<section aria-labelledby="security-panel-heading">
  <h3 id="security-panel-heading">Security & Authentication</h3>
  
  <div class="security-layout">
    <!-- Tabs -->
    <div class="security-tabs">
      <button 
        class="tab-button"
        class:active={activeTab === 'users'}
        on:click={() => activeTab = 'users'}
      >
        Users ({users.length})
      </button>
      <button 
        class="tab-button"
        class:active={activeTab === 'roles'}
        on:click={() => activeTab = 'roles'}
      >
        Roles ({roles.length})
      </button>
      <button 
        class="tab-button"
        class:active={activeTab === 'policies'}
        on:click={() => activeTab = 'policies'}
      >
        Policies ({policies.length})
      </button>
      <button 
        class="tab-button"
        class:active={activeTab === 'tokens'}
        on:click={() => activeTab = 'tokens'}
      >
        Tokens ({tokens.length})
      </button>
      <button 
        class="tab-button"
        class:active={activeTab === 'settings'}
        on:click={() => activeTab = 'settings'}
      >
        Settings
      </button>
    </div>
    
    <!-- Content -->
    <div class="security-content">
      {#if activeTab === 'users'}
        <div class="users-section">
          <div class="section-header">
            <h4>Users</h4>
            <button on:click={() => showUserDialog = true} class="primary">
              Add User
            </button>
          </div>
          
          <div class="users-list">
            {#each users as user}
              <div 
                class="user-item"
                class:selected={selectedUser?.id === user.id}
                role="button"
                tabindex="0"
                on:click={() => selectUser(user)}
                on:keydown={(e) => e.key === 'Enter' && selectUser(user)}
              >
                <div class="user-header">
                  <div class="user-info">
                    <span class="user-name">{user.username}</span>
                    <span class="user-email">{user.email}</span>
                    <span class="user-role">{user.role}</span>
                  </div>
                  <div class="user-status" style="color: {getStatusColor(user.status)}">
                    {getStatusIcon(user.status)} {user.status}
                  </div>
                </div>
                <div class="user-details">
                  <span>Last login: {user.lastLogin ? formatTimestamp(user.lastLogin) : 'Never'}</span>
                  <span>Created: {formatTimestamp(user.createdAt)}</span>
                </div>
                <div class="user-actions">
                  <button on:click|stopPropagation={() => deleteUser(user.id)} class="small">
                    Delete
                  </button>
                </div>
              </div>
            {/each}
          </div>
        </div>
      {:else if activeTab === 'roles'}
        <div class="roles-section">
          <div class="section-header">
            <h4>Roles</h4>
            <button on:click={() => showRoleDialog = true} class="primary">
              Add Role
            </button>
          </div>
          
          <div class="roles-list">
            {#each roles as role}
              <div 
                class="role-item"
                class:selected={selectedRole?.id === role.id}
                role="button"
                tabindex="0"
                on:click={() => selectRole(role)}
                on:keydown={(e) => e.key === 'Enter' && selectRole(role)}
              >
                <div class="role-header">
                  <div class="role-info">
                    <span class="role-name">{role.name}</span>
                    <span class="role-description">{role.description}</span>
                  </div>
                  <div class="role-meta">
                    <span class="role-users">{role.userCount} users</span>
                  </div>
                </div>
                <div class="role-permissions">
                  {#each role.permissions as permission}
                    <span class="permission-tag">{permission}</span>
                  {/each}
                </div>
                <div class="role-actions">
                  <button on:click|stopPropagation={() => deleteRole(role.id)} class="small">
                    Delete
                  </button>
                </div>
              </div>
            {/each}
          </div>
        </div>
      {:else if activeTab === 'policies'}
        <div class="policies-section">
          <div class="section-header">
            <h4>Policies</h4>
            <button on:click={() => showPolicyDialog = true} class="primary">
              Add Policy
            </button>
          </div>
          
          <div class="policies-list">
            {#each policies as policy}
              <div 
                class="policy-item"
                class:selected={selectedPolicy?.id === policy.id}
                role="button"
                tabindex="0"
                on:click={() => selectPolicy(policy)}
                on:keydown={(e) => e.key === 'Enter' && selectPolicy(policy)}
              >
                <div class="policy-header">
                  <div class="policy-info">
                    <span class="policy-name">{policy.name}</span>
                    <span class="policy-description">{policy.description}</span>
                  </div>
                  <div class="policy-meta">
                    <span class="policy-effect" class:allow={policy.effect === 'allow'} class:deny={policy.effect === 'deny'}>
                      {policy.effect}
                    </span>
                    <span class="policy-priority">Priority: {policy.priority}</span>
                  </div>
                </div>
                <div class="policy-details">
                  <span>Resource: {policy.resource}</span>
                  <span>Actions: {policy.actions.join(', ')}</span>
                  <span>Conditions: {policy.conditions.join(', ')}</span>
                </div>
                <div class="policy-actions">
                  <button on:click|stopPropagation={() => deletePolicy(policy.id)} class="small">
                    Delete
                  </button>
                </div>
              </div>
            {/each}
          </div>
        </div>
      {:else if activeTab === 'tokens'}
        <div class="tokens-section">
          <div class="section-header">
            <h4>API Tokens</h4>
            <button on:click={() => showTokenDialog = true} class="primary">
              Add Token
            </button>
          </div>
          
          <div class="tokens-list">
            {#each tokens as token}
              <div 
                class="token-item"
                class:selected={selectedToken?.id === token.id}
                role="button"
                tabindex="0"
                on:click={() => selectToken(token)}
                on:keydown={(e) => e.key === 'Enter' && selectToken(token)}
              >
                <div class="token-header">
                  <div class="token-info">
                    <span class="token-name">{token.name}</span>
                    <span class="token-value">{token.token}</span>
                  </div>
                  <div class="token-status" style="color: {getStatusColor(token.status)}">
                    {getStatusIcon(token.status)} {token.status}
                  </div>
                </div>
                <div class="token-details">
                  <span>Expires: {formatTimestamp(token.expiresAt)}</span>
                  <span>Last used: {token.lastUsed ? formatTimestamp(token.lastUsed) : 'Never'}</span>
                </div>
                <div class="token-permissions">
                  {#each token.permissions as permission}
                    <span class="permission-tag">{permission}</span>
                  {/each}
                </div>
                <div class="token-actions">
                  <button on:click|stopPropagation={() => revokeToken(token.id)} class="small">
                    Revoke
                  </button>
                </div>
              </div>
            {/each}
          </div>
        </div>
      {:else if activeTab === 'settings'}
        <div class="settings-section">
          <h4>Security Settings</h4>
          <div class="settings-grid">
            <div class="setting-item">
              <label>Session Timeout</label>
              <input type="number" value="3600" min="300" max="86400" />
              <span>seconds</span>
            </div>
            <div class="setting-item">
              <label>Password Policy</label>
              <select>
                <option value="basic">Basic (8+ characters)</option>
                <option value="strong">Strong (12+ chars, mixed case, numbers)</option>
                <option value="complex">Complex (16+ chars, special chars)</option>
              </select>
            </div>
            <div class="setting-item">
              <label>Two-Factor Authentication</label>
              <input type="checkbox" />
            </div>
            <div class="setting-item">
              <label>API Rate Limiting</label>
              <input type="number" value="1000" min="100" max="10000" />
              <span>requests per hour</span>
            </div>
          </div>
        </div>
      {/if}
    </div>
  </div>
  
  <!-- User Dialog -->
  {#if showUserDialog}
    <div class="dialog-overlay">
      <div class="dialog">
        <h4>Add User</h4>
        <input 
          type="text" 
          bind:value={newUser.username} 
          placeholder="Username..."
        />
        <input 
          type="email" 
          bind:value={newUser.email} 
          placeholder="Email..."
        />
        <select bind:value={newUser.role}>
          {#each roles as role}
            <option value={role.name}>{role.name}</option>
          {/each}
        </select>
        <input 
          type="password" 
          bind:value={newUser.password} 
          placeholder="Password..."
        />
        <div class="dialog-actions">
          <button on:click={createUser} disabled={!newUser.username.trim() || !newUser.email.trim()}>
            Create
          </button>
          <button on:click={() => showUserDialog = false} class="secondary">
            Cancel
          </button>
        </div>
      </div>
    </div>
  {/if}
  
  <!-- Role Dialog -->
  {#if showRoleDialog}
    <div class="dialog-overlay">
      <div class="dialog">
        <h4>Add Role</h4>
        <input 
          type="text" 
          bind:value={newRole.name} 
          placeholder="Role name..."
        />
        <textarea 
          bind:value={newRole.description} 
          placeholder="Description..."
          rows="3"
        ></textarea>
        <div class="permissions-grid">
          {#each availablePermissions as permission}
            <label>
              <input 
                type="checkbox" 
                bind:group={newRole.permissions} 
                value={permission}
              />
              {permission}
            </label>
          {/each}
        </div>
        <div class="dialog-actions">
          <button on:click={createRole} disabled={!newRole.name.trim()}>
            Create
          </button>
          <button on:click={() => showRoleDialog = false} class="secondary">
            Cancel
          </button>
        </div>
      </div>
    </div>
  {/if}
  
  <!-- Policy Dialog -->
  {#if showPolicyDialog}
    <div class="dialog-overlay">
      <div class="dialog">
        <h4>Add Policy</h4>
        <input 
          type="text" 
          bind:value={newPolicy.name} 
          placeholder="Policy name..."
        />
        <textarea 
          bind:value={newPolicy.description} 
          placeholder="Description..."
          rows="3"
        ></textarea>
        <select bind:value={newPolicy.resource}>
          {#each availableResources as resource}
            <option value={resource}>{resource}</option>
          {/each}
        </select>
        <div class="actions-grid">
          {#each availableActions as action}
            <label>
              <input 
                type="checkbox" 
                bind:group={newPolicy.actions} 
                value={action}
              />
              {action}
            </label>
          {/each}
        </div>
        <div class="dialog-actions">
          <button on:click={createPolicy} disabled={!newPolicy.name.trim() || !newPolicy.resource.trim()}>
            Create
          </button>
          <button on:click={() => showPolicyDialog = false} class="secondary">
            Cancel
          </button>
        </div>
      </div>
    </div>
  {/if}
  
  <!-- Token Dialog -->
  {#if showTokenDialog}
    <div class="dialog-overlay">
      <div class="dialog">
        <h4>Add Token</h4>
        <input 
          type="text" 
          bind:value={newToken.name} 
          placeholder="Token name..."
        />
        <input 
          type="number" 
          bind:value={newToken.expiresIn} 
          min="1" 
          max="365"
        />
        <span>days</span>
        <div class="permissions-grid">
          {#each availablePermissions as permission}
            <label>
              <input 
                type="checkbox" 
                bind:group={newToken.permissions} 
                value={permission}
              />
              {permission}
            </label>
          {/each}
        </div>
        <div class="dialog-actions">
          <button on:click={createToken} disabled={!newToken.name.trim()}>
            Create
          </button>
          <button on:click={() => showTokenDialog = false} class="secondary">
            Cancel
          </button>
        </div>
      </div>
    </div>
  {/if}
</section>

<style>
  .security-layout {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  
  .security-tabs {
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
  
  .security-content {
    flex: 1;
    padding: 1rem;
    background: var(--pico-background-color);
    border-radius: 8px;
    border: 1px solid var(--pico-muted-border-color);
  }
  
  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }
  
  .users-list, .roles-list, .policies-list, .tokens-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }
  
  .user-item, .role-item, .policy-item, .token-item {
    padding: 1rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    background: var(--pico-muted-border-color);
    cursor: pointer;
    transition: all 0.2s;
  }
  
  .user-item:hover, .role-item:hover, .policy-item:hover, .token-item:hover {
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
  }
  
  .user-item.selected, .role-item.selected, .policy-item.selected, .token-item.selected {
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
  }
  
  .user-header, .role-header, .policy-header, .token-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
  }
  
  .user-info, .role-info, .policy-info, .token-info {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  
  .user-name, .role-name, .policy-name, .token-name {
    font-weight: 600;
    font-size: 1rem;
  }
  
  .user-email, .role-description, .policy-description {
    font-size: 0.875rem;
    opacity: 0.8;
  }
  
  .user-role, .role-users, .policy-effect, .token-value {
    font-size: 0.875rem;
    color: var(--pico-primary);
  }
  
  .policy-effect.allow {
    color: var(--success-color);
  }
  
  .policy-effect.deny {
    color: var(--error-color);
  }
  
  .user-details, .policy-details, .token-details {
    display: flex;
    gap: 1rem;
    font-size: 0.875rem;
    opacity: 0.8;
    margin-bottom: 0.5rem;
  }
  
  .role-permissions, .token-permissions {
    display: flex;
    gap: 0.25rem;
    flex-wrap: wrap;
    margin-bottom: 0.5rem;
  }
  
  .permission-tag {
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
    padding: 0.125rem 0.375rem;
    border-radius: 3px;
    font-size: 0.75rem;
  }
  
  .user-actions, .role-actions, .policy-actions, .token-actions {
    display: flex;
    gap: 0.5rem;
  }
  
  .settings-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: 1rem;
  }
  
  .setting-item {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  
  .setting-item label {
    font-weight: 600;
  }
  
  .setting-item input,
  .setting-item select {
    width: 100%;
  }
  
  .permissions-grid, .actions-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 0.5rem;
    max-height: 200px;
    overflow-y: auto;
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
    min-width: 500px;
    max-width: 600px;
  }
  
  .dialog h4 {
    margin-bottom: 1rem;
  }
  
  .dialog input,
  .dialog textarea,
  .dialog select {
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
  
  @media (max-width: 768px) {
    .security-tabs {
      flex-wrap: wrap;
    }
    
    .tab-button {
      flex: 1;
      min-width: 120px;
    }
    
    .settings-grid {
      grid-template-columns: 1fr;
    }
    
    .permissions-grid, .actions-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
