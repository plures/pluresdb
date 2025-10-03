<script lang="ts">
  import { onMount } from 'svelte'
  import { push as toast } from '../lib/toasts'

  // Data Sharing
  let sharedNodes = [] as any[]
  let receivedNodes = [] as any[]
  let sharingForm = {
    nodeId: '',
    targetPeerId: '',
    message: '',
    encryptionType: 'public-key',
    expiration: 'never',
    accessLevel: 'read-only'
  }

  // Encryption Keys
  let encryptionKeys = [] as any[]
  let keyForm = {
    name: '',
    type: 'rsa',
    keySize: 2048,
    description: ''
  }

  // Access Control
  let accessPolicies = [] as any[]
  let policyForm = {
    name: '',
    description: '',
    rules: [] as any[],
    isActive: true
  }

  // Sharing History
  let sharingHistory = [] as any[]

  onMount(() => {
    loadSharedNodes()
    loadReceivedNodes()
    loadEncryptionKeys()
    loadAccessPolicies()
    loadSharingHistory()
  })

  function loadSharedNodes() {
    const stored = localStorage.getItem('pluresdb-shared-nodes')
    if (stored) {
      sharedNodes = JSON.parse(stored)
    }
  }

  function loadReceivedNodes() {
    const stored = localStorage.getItem('pluresdb-received-nodes')
    if (stored) {
      receivedNodes = JSON.parse(stored)
    }
  }

  function loadEncryptionKeys() {
    const stored = localStorage.getItem('pluresdb-encryption-keys')
    if (stored) {
      encryptionKeys = JSON.parse(stored)
    } else {
      // Generate default keys
      generateDefaultKeys()
    }
  }

  function loadAccessPolicies() {
    const stored = localStorage.getItem('pluresdb-access-policies')
    if (stored) {
      accessPolicies = JSON.parse(stored)
    } else {
      // Create default policies
      createDefaultPolicies()
    }
  }

  function loadSharingHistory() {
    const stored = localStorage.getItem('pluresdb-sharing-history')
    if (stored) {
      sharingHistory = JSON.parse(stored)
    }
  }

  function generateDefaultKeys() {
    encryptionKeys = [
      {
        id: 'key_1',
        name: 'My Primary Key',
        type: 'rsa',
        keySize: 2048,
        publicKey: 'pk_primary_123456789',
        privateKey: 'sk_primary_123456789',
        createdAt: new Date(),
        isActive: true
      },
      {
        id: 'key_2',
        name: 'Backup Key',
        type: 'rsa',
        keySize: 2048,
        publicKey: 'pk_backup_987654321',
        privateKey: 'sk_backup_987654321',
        createdAt: new Date(),
        isActive: false
      }
    ]
    saveEncryptionKeys()
  }

  function createDefaultPolicies() {
    accessPolicies = [
      {
        id: 'policy_1',
        name: 'Public Data',
        description: 'Allow anyone to read this data',
        rules: [
          { type: 'allow', action: 'read', condition: 'always' }
        ],
        isActive: true
      },
      {
        id: 'policy_2',
        name: 'Friends Only',
        description: 'Only allow friends to read and write',
        rules: [
          { type: 'allow', action: 'read', condition: 'is_friend' },
          { type: 'allow', action: 'write', condition: 'is_friend' }
        ],
        isActive: true
      },
      {
        id: 'policy_3',
        name: 'Encrypted Private',
        description: 'Require encryption and specific peer',
        rules: [
          { type: 'require', action: 'encryption', condition: 'always' },
          { type: 'allow', action: 'read', condition: 'has_valid_key' }
        ],
        isActive: true
      }
    ]
    saveAccessPolicies()
  }

  function saveEncryptionKeys() {
    localStorage.setItem('pluresdb-encryption-keys', JSON.stringify(encryptionKeys))
  }

  function saveAccessPolicies() {
    localStorage.setItem('pluresdb-access-policies', JSON.stringify(accessPolicies))
  }

  function saveSharedNodes() {
    localStorage.setItem('pluresdb-shared-nodes', JSON.stringify(sharedNodes))
  }

  function saveReceivedNodes() {
    localStorage.setItem('pluresdb-received-nodes', JSON.stringify(receivedNodes))
  }

  function saveSharingHistory() {
    localStorage.setItem('pluresdb-sharing-history', JSON.stringify(sharingHistory))
  }

  function generateKey() {
    const key = {
      id: 'key_' + Math.random().toString(36).substr(2, 9),
      name: keyForm.name || 'New Key',
      type: keyForm.type,
      keySize: keyForm.keySize,
      publicKey: 'pk_' + Math.random().toString(36).substr(2, 16),
      privateKey: 'sk_' + Math.random().toString(36).substr(2, 16),
      description: keyForm.description,
      createdAt: new Date(),
      isActive: true
    }
    
    encryptionKeys.push(key)
    saveEncryptionKeys()
    
    // Reset form
    keyForm = {
      name: '',
      type: 'rsa',
      keySize: 2048,
      description: ''
    }
    
    toast.success('Encryption key generated')
  }

  function createPolicy() {
    const policy = {
      id: 'policy_' + Math.random().toString(36).substr(2, 9),
      name: policyForm.name,
      description: policyForm.description,
      rules: policyForm.rules,
      isActive: policyForm.isActive,
      createdAt: new Date()
    }
    
    accessPolicies.push(policy)
    saveAccessPolicies()
    
    // Reset form
    policyForm = {
      name: '',
      description: '',
      rules: [],
      isActive: true
    }
    
    toast.success('Access policy created')
  }

  function addPolicyRule() {
    policyForm.rules.push({
      type: 'allow',
      action: 'read',
      condition: 'always'
    })
  }

  function removePolicyRule(index: number) {
    policyForm.rules.splice(index, 1)
  }

  function shareNode() {
    if (!sharingForm.nodeId || !sharingForm.targetPeerId) {
      toast.error('Please fill in all required fields')
      return
    }

    const sharedNode = {
      id: 'shared_' + Math.random().toString(36).substr(2, 9),
      nodeId: sharingForm.nodeId,
      targetPeerId: sharingForm.targetPeerId,
      message: sharingForm.message,
      encryptionType: sharingForm.encryptionType,
      expiration: sharingForm.expiration,
      accessLevel: sharingForm.accessLevel,
      status: 'pending',
      createdAt: new Date(),
      encryptedData: 'encrypted_' + Math.random().toString(36).substr(2, 16)
    }

    sharedNodes.push(sharedNode)
    saveSharedNodes()

    // Add to history
    sharingHistory.push({
      id: 'history_' + Math.random().toString(36).substr(2, 9),
      type: 'shared',
      nodeId: sharingForm.nodeId,
      targetPeerId: sharingForm.targetPeerId,
      timestamp: new Date(),
      status: 'success'
    })
    saveSharingHistory()

    // Reset form
    sharingForm = {
      nodeId: '',
      targetPeerId: '',
      message: '',
      encryptionType: 'public-key',
      expiration: 'never',
      accessLevel: 'read-only'
    }

    toast.success('Node shared successfully')
  }

  function revokeAccess(sharedNodeId: string) {
    const node = sharedNodes.find(n => n.id === sharedNodeId)
    if (node) {
      node.status = 'revoked'
      node.revokedAt = new Date()
      saveSharedNodes()
      toast.success('Access revoked')
    }
  }

  function acceptReceivedNode(receivedNodeId: string) {
    const node = receivedNodes.find(n => n.id === receivedNodeId)
    if (node) {
      node.status = 'accepted'
      node.acceptedAt = new Date()
      saveReceivedNodes()
      toast.success('Received node accepted')
    }
  }

  function rejectReceivedNode(receivedNodeId: string) {
    const node = receivedNodes.find(n => n.id === receivedNodeId)
    if (node) {
      node.status = 'rejected'
      node.rejectedAt = new Date()
      saveReceivedNodes()
      toast.info('Received node rejected')
    }
  }

  function deleteKey(keyId: string) {
    encryptionKeys = encryptionKeys.filter(k => k.id !== keyId)
    saveEncryptionKeys()
    toast.info('Encryption key deleted')
  }

  function toggleKeyActive(keyId: string) {
    const key = encryptionKeys.find(k => k.id === keyId)
    if (key) {
      key.isActive = !key.isActive
      saveEncryptionKeys()
      toast.success(`Key ${key.isActive ? 'activated' : 'deactivated'}`)
    }
  }

  function deletePolicy(policyId: string) {
    accessPolicies = accessPolicies.filter(p => p.id !== policyId)
    saveAccessPolicies()
    toast.info('Access policy deleted')
  }

  function togglePolicyActive(policyId: string) {
    const policy = accessPolicies.find(p => p.id === policyId)
    if (policy) {
      policy.isActive = !policy.isActive
      saveAccessPolicies()
      toast.success(`Policy ${policy.isActive ? 'activated' : 'deactivated'}`)
    }
  }
</script>

<div class="encrypted-sharing">
  <div class="header">
    <h2>Encrypted Data Sharing</h2>
    <p>Securely share encrypted data with peers in the P2P network</p>
  </div>

  <div class="tabs">
    <button class="tab active">Share Data</button>
    <button class="tab">Received Data</button>
    <button class="tab">Encryption Keys</button>
    <button class="tab">Access Policies</button>
    <button class="tab">History</button>
  </div>

  <div class="tab-content">
    <!-- Share Data Tab -->
    <div class="tab-panel active">
      <div class="sharing-form">
        <h3>Share a Node</h3>
        <div class="form-group">
          <label for="node-id">Node ID</label>
          <input 
            id="node-id"
            type="text" 
            bind:value={sharingForm.nodeId}
            placeholder="Enter the ID of the node to share"
          />
        </div>

        <div class="form-group">
          <label for="target-peer">Target Peer ID</label>
          <input 
            id="target-peer"
            type="text" 
            bind:value={sharingForm.targetPeerId}
            placeholder="Enter the peer ID to share with"
          />
        </div>

        <div class="form-group">
          <label for="message">Message</label>
          <textarea 
            id="message"
            bind:value={sharingForm.message}
            placeholder="Optional message for the peer"
            rows="3"
          ></textarea>
        </div>

        <div class="form-row">
          <div class="form-group">
            <label for="encryption-type">Encryption Type</label>
            <select id="encryption-type" bind:value={sharingForm.encryptionType}>
              <option value="public-key">Public Key</option>
              <option value="symmetric">Symmetric</option>
              <option value="hybrid">Hybrid</option>
            </select>
          </div>

          <div class="form-group">
            <label for="access-level">Access Level</label>
            <select id="access-level" bind:value={sharingForm.accessLevel}>
              <option value="read-only">Read Only</option>
              <option value="read-write">Read/Write</option>
              <option value="admin">Admin</option>
            </select>
          </div>
        </div>

        <div class="form-group">
          <label for="expiration">Expiration</label>
          <select id="expiration" bind:value={sharingForm.expiration}>
            <option value="never">Never</option>
            <option value="1hour">1 Hour</option>
            <option value="1day">1 Day</option>
            <option value="1week">1 Week</option>
            <option value="1month">1 Month</option>
          </select>
        </div>

        <button class="btn btn-primary" on:click={shareNode}>
          Share Node
        </button>
      </div>

      <div class="shared-nodes">
        <h3>Shared Nodes</h3>
        {#if sharedNodes.length === 0}
          <p class="empty">No nodes shared yet</p>
        {:else}
          {#each sharedNodes as node}
            <div class="node-card">
              <div class="node-info">
                <h4>Node: {node.nodeId}</h4>
                <p>Target: {node.targetPeerId}</p>
                <p>Encryption: {node.encryptionType}</p>
                <p>Access: {node.accessLevel}</p>
                <p>Status: <span class="status {node.status}">{node.status}</span></p>
                <p class="timestamp">Shared: {node.createdAt.toLocaleString()}</p>
              </div>
              <div class="node-actions">
                {#if node.status === 'active'}
                  <button 
                    class="btn btn-danger"
                    on:click={() => revokeAccess(node.id)}
                  >
                    Revoke
                  </button>
                {/if}
              </div>
            </div>
          {/each}
        {/if}
      </div>
    </div>

    <!-- Received Data Tab -->
    <div class="tab-panel">
      <div class="received-nodes">
        <h3>Received Nodes</h3>
        {#if receivedNodes.length === 0}
          <p class="empty">No received nodes</p>
        {:else}
          {#each receivedNodes as node}
            <div class="node-card">
              <div class="node-info">
                <h4>Node: {node.nodeId}</h4>
                <p>From: {node.fromPeerId}</p>
                <p>Message: {node.message}</p>
                <p>Encryption: {node.encryptionType}</p>
                <p>Status: <span class="status {node.status}">{node.status}</span></p>
                <p class="timestamp">Received: {node.receivedAt.toLocaleString()}</p>
              </div>
              <div class="node-actions">
                {#if node.status === 'pending'}
                  <button 
                    class="btn btn-success"
                    on:click={() => acceptReceivedNode(node.id)}
                  >
                    Accept
                  </button>
                  <button 
                    class="btn btn-danger"
                    on:click={() => rejectReceivedNode(node.id)}
                  >
                    Reject
                  </button>
                {/if}
              </div>
            </div>
          {/each}
        {/if}
      </div>
    </div>

    <!-- Encryption Keys Tab -->
    <div class="tab-panel">
      <div class="key-form">
        <h3>Generate New Key</h3>
        <div class="form-group">
          <label for="key-name">Key Name</label>
          <input 
            id="key-name"
            type="text" 
            bind:value={keyForm.name}
            placeholder="Enter a name for this key"
          />
        </div>

        <div class="form-row">
          <div class="form-group">
            <label for="key-type">Key Type</label>
            <select id="key-type" bind:value={keyForm.type}>
              <option value="rsa">RSA</option>
              <option value="ecdsa">ECDSA</option>
              <option value="ed25519">Ed25519</option>
            </select>
          </div>

          <div class="form-group">
            <label for="key-size">Key Size</label>
            <select id="key-size" bind:value={keyForm.keySize}>
              <option value={2048}>2048 bits</option>
              <option value={3072}>3072 bits</option>
              <option value={4096}>4096 bits</option>
            </select>
          </div>
        </div>

        <div class="form-group">
          <label for="key-description">Description</label>
          <textarea 
            id="key-description"
            bind:value={keyForm.description}
            placeholder="Optional description for this key"
            rows="2"
          ></textarea>
        </div>

        <button class="btn btn-primary" on:click={generateKey}>
          Generate Key
        </button>
      </div>

      <div class="encryption-keys">
        <h3>Encryption Keys</h3>
        {#if encryptionKeys.length === 0}
          <p class="empty">No encryption keys</p>
        {:else}
          {#each encryptionKeys as key}
            <div class="key-card">
              <div class="key-info">
                <h4>{key.name}</h4>
                <p>Type: {key.type.toUpperCase()}</p>
                <p>Size: {key.keySize} bits</p>
                <p>Public Key: {key.publicKey}</p>
                <p>Status: <span class="status {key.isActive ? 'active' : 'inactive'}">{key.isActive ? 'Active' : 'Inactive'}</span></p>
                <p class="timestamp">Created: {key.createdAt.toLocaleString()}</p>
              </div>
              <div class="key-actions">
                <button 
                  class="btn btn-secondary"
                  on:click={() => toggleKeyActive(key.id)}
                >
                  {key.isActive ? 'Deactivate' : 'Activate'}
                </button>
                <button 
                  class="btn btn-danger"
                  on:click={() => deleteKey(key.id)}
                >
                  Delete
                </button>
              </div>
            </div>
          {/each}
        {/if}
      </div>
    </div>

    <!-- Access Policies Tab -->
    <div class="tab-panel">
      <div class="policy-form">
        <h3>Create Access Policy</h3>
        <div class="form-group">
          <label for="policy-name">Policy Name</label>
          <input 
            id="policy-name"
            type="text" 
            bind:value={policyForm.name}
            placeholder="Enter a name for this policy"
          />
        </div>

        <div class="form-group">
          <label for="policy-description">Description</label>
          <textarea 
            id="policy-description"
            bind:value={policyForm.description}
            placeholder="Describe what this policy does"
            rows="2"
          ></textarea>
        </div>

        <div class="form-group">
          <label>Rules</label>
          {#each policyForm.rules as rule, index}
            <div class="rule-item">
              <select bind:value={rule.type}>
                <option value="allow">Allow</option>
                <option value="deny">Deny</option>
                <option value="require">Require</option>
              </select>
              <select bind:value={rule.action}>
                <option value="read">Read</option>
                <option value="write">Write</option>
                <option value="delete">Delete</option>
                <option value="encryption">Encryption</option>
              </select>
              <select bind:value={rule.condition}>
                <option value="always">Always</option>
                <option value="is_friend">Is Friend</option>
                <option value="has_valid_key">Has Valid Key</option>
                <option value="is_owner">Is Owner</option>
              </select>
              <button 
                type="button" 
                class="btn btn-danger"
                on:click={() => removePolicyRule(index)}
              >
                Remove
              </button>
            </div>
          {/each}
          <button 
            type="button" 
            class="btn btn-secondary"
            on:click={addPolicyRule}
          >
            Add Rule
          </button>
        </div>

        <div class="form-group">
          <label>
            <input 
              type="checkbox" 
              bind:checked={policyForm.isActive}
            />
            Active
          </label>
        </div>

        <button class="btn btn-primary" on:click={createPolicy}>
          Create Policy
        </button>
      </div>

      <div class="access-policies">
        <h3>Access Policies</h3>
        {#if accessPolicies.length === 0}
          <p class="empty">No access policies</p>
        {:else}
          {#each accessPolicies as policy}
            <div class="policy-card">
              <div class="policy-info">
                <h4>{policy.name}</h4>
                <p>{policy.description}</p>
                <div class="rules">
                  {#each policy.rules as rule}
                    <span class="rule">
                      {rule.type} {rule.action} {rule.condition}
                    </span>
                  {/each}
                </div>
                <p>Status: <span class="status {policy.isActive ? 'active' : 'inactive'}">{policy.isActive ? 'Active' : 'Inactive'}</span></p>
                <p class="timestamp">Created: {policy.createdAt.toLocaleString()}</p>
              </div>
              <div class="policy-actions">
                <button 
                  class="btn btn-secondary"
                  on:click={() => togglePolicyActive(policy.id)}
                >
                  {policy.isActive ? 'Deactivate' : 'Activate'}
                </button>
                <button 
                  class="btn btn-danger"
                  on:click={() => deletePolicy(policy.id)}
                >
                  Delete
                </button>
              </div>
            </div>
          {/each}
        {/if}
      </div>
    </div>

    <!-- History Tab -->
    <div class="tab-panel">
      <div class="sharing-history">
        <h3>Sharing History</h3>
        {#if sharingHistory.length === 0}
          <p class="empty">No sharing history</p>
        {:else}
          {#each sharingHistory as entry}
            <div class="history-item">
              <div class="history-info">
                <h4>{entry.type === 'shared' ? 'Shared' : 'Received'}</h4>
                <p>Node: {entry.nodeId}</p>
                <p>Peer: {entry.targetPeerId || entry.fromPeerId}</p>
                <p>Status: <span class="status {entry.status}">{entry.status}</span></p>
                <p class="timestamp">{entry.timestamp.toLocaleString()}</p>
              </div>
            </div>
          {/each}
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  .encrypted-sharing {
    padding: 1rem;
    max-width: 1200px;
    margin: 0 auto;
  }

  .header {
    margin-bottom: 2rem;
  }

  .header h2 {
    margin: 0 0 0.5rem 0;
    color: var(--primary);
  }

  .header p {
    margin: 0;
    color: var(--muted);
  }

  .tabs {
    display: flex;
    border-bottom: 1px solid var(--border);
    margin-bottom: 2rem;
  }

  .tab {
    padding: 0.75rem 1.5rem;
    border: none;
    background: none;
    cursor: pointer;
    border-bottom: 2px solid transparent;
    transition: all 0.2s;
  }

  .tab:hover {
    background: var(--hover);
  }

  .tab.active {
    border-bottom-color: var(--primary);
    color: var(--primary);
  }

  .tab-content {
    min-height: 400px;
  }

  .tab-panel {
    display: none;
  }

  .tab-panel.active {
    display: block;
  }

  .sharing-form,
  .key-form,
  .policy-form {
    max-width: 600px;
    margin-bottom: 2rem;
  }

  .form-group {
    margin-bottom: 1.5rem;
  }

  .form-group label {
    display: block;
    margin-bottom: 0.5rem;
    font-weight: 500;
  }

  .form-group input,
  .form-group textarea,
  .form-group select {
    width: 100%;
    padding: 0.75rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    font-size: 1rem;
  }

  .form-group textarea {
    resize: vertical;
    min-height: 80px;
  }

  .form-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
  }

  .rule-item {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 0.5rem;
    align-items: center;
  }

  .rule-item select {
    flex: 1;
  }

  .rules {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    margin: 0.5rem 0;
  }

  .rule {
    padding: 0.25rem 0.75rem;
    background: var(--secondary);
    color: var(--text);
    border-radius: 20px;
    font-size: 0.875rem;
  }

  .node-card,
  .key-card,
  .policy-card {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    margin-bottom: 1rem;
  }

  .node-info,
  .key-info,
  .policy-info h4 {
    margin: 0 0 0.5rem 0;
  }

  .node-info p,
  .key-info p,
  .policy-info p {
    margin: 0.25rem 0;
    color: var(--muted);
  }

  .node-info .timestamp,
  .key-info .timestamp,
  .policy-info .timestamp {
    font-size: 0.875rem;
  }

  .node-actions,
  .key-actions,
  .policy-actions {
    display: flex;
    gap: 0.5rem;
  }

  .status {
    padding: 0.25rem 0.75rem;
    border-radius: 20px;
    font-size: 0.875rem;
    font-weight: 500;
  }

  .status.active {
    background: var(--success);
    color: white;
  }

  .status.inactive {
    background: var(--muted);
    color: white;
  }

  .status.pending {
    background: var(--warning);
    color: white;
  }

  .status.success {
    background: var(--success);
    color: white;
  }

  .status.error {
    background: var(--danger);
    color: white;
  }

  .history-item {
    padding: 1rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    margin-bottom: 1rem;
  }

  .history-info h4 {
    margin: 0 0 0.5rem 0;
  }

  .history-info p {
    margin: 0.25rem 0;
    color: var(--muted);
  }

  .history-info .timestamp {
    font-size: 0.875rem;
  }

  .empty {
    text-align: center;
    color: var(--muted);
    font-style: italic;
    padding: 2rem;
  }

  .btn {
    padding: 0.5rem 1rem;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.875rem;
    font-weight: 500;
    transition: all 0.2s;
  }

  .btn-primary {
    background: var(--primary);
    color: white;
  }

  .btn-primary:hover {
    background: var(--primary-dark);
  }

  .btn-secondary {
    background: var(--secondary);
    color: var(--text);
  }

  .btn-secondary:hover {
    background: var(--secondary-dark);
  }

  .btn-success {
    background: var(--success);
    color: white;
  }

  .btn-success:hover {
    background: var(--success-dark);
  }

  .btn-danger {
    background: var(--danger);
    color: white;
  }

  .btn-danger:hover {
    background: var(--danger-dark);
  }
</style>
