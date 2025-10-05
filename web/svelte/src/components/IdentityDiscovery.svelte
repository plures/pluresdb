<script lang="ts">
  import { onMount } from "svelte";
  import { push as toast } from "../lib/toasts";

  // Identity Management
  let identity = {
    id: "",
    publicKey: "",
    name: "",
    email: "",
    phone: "",
    picture: "",
    bio: "",
    location: "",
    tags: [] as string[],
    createdAt: new Date(),
    lastSeen: new Date(),
  };

  // Peer Discovery
  let discoveredPeers = [] as any[];
  let searchQuery = "";
  let searchResults = [] as any[];
  let isSearching = false;

  // Acceptance Policies
  let acceptancePolicies = {
    default: {
      allowDataSharing: true,
      allowFileSharing: false,
      allowLocationSharing: false,
      maxDataSize: "10MB",
      allowedTypes: ["text", "json", "image"],
      requireApproval: false,
    },
    laptop: {
      allowDataSharing: true,
      allowFileSharing: true,
      allowLocationSharing: false,
      maxDataSize: "100MB",
      allowedTypes: ["text", "json", "image", "video", "audio"],
      requireApproval: false,
    },
    phone: {
      allowDataSharing: true,
      allowFileSharing: false,
      allowLocationSharing: true,
      maxDataSize: "50MB",
      allowedTypes: ["text", "json", "image"],
      requireApproval: true,
    },
    server: {
      allowDataSharing: true,
      allowFileSharing: true,
      allowLocationSharing: false,
      maxDataSize: "1GB",
      allowedTypes: ["text", "json", "image", "video", "audio", "database"],
      requireApproval: false,
    },
  };

  let currentPolicy = "default";
  let customPolicy = {
    allowDataSharing: true,
    allowFileSharing: false,
    allowLocationSharing: false,
    maxDataSize: "10MB",
    allowedTypes: ["text", "json"],
    requireApproval: true,
  };

  // Peer Requests
  let pendingRequests = [] as any[];
  let sentRequests = [] as any[];

  onMount(() => {
    loadIdentity();
    loadDiscoveredPeers();
    loadPendingRequests();
  });

  function loadIdentity() {
    // Load from local storage or generate new
    const stored = localStorage.getItem("pluresdb-identity");
    if (stored) {
      identity = JSON.parse(stored);
    } else {
      generateNewIdentity();
    }
  }

  function generateNewIdentity() {
    // Generate a new identity with public key
    identity.id = generateId();
    identity.publicKey = generatePublicKey();
    identity.name = "Anonymous User";
    identity.createdAt = new Date();
    identity.lastSeen = new Date();
    saveIdentity();
  }

  function generateId(): string {
    return "id_" + Math.random().toString(36).substr(2, 9);
  }

  function generatePublicKey(): string {
    // In a real implementation, this would generate an actual public key
    return "pk_" + Math.random().toString(36).substr(2, 16);
  }

  function saveIdentity() {
    localStorage.setItem("pluresdb-identity", JSON.stringify(identity));
    toast.success("Identity saved");
  }

  function loadDiscoveredPeers() {
    // Load discovered peers from local storage
    const stored = localStorage.getItem("pluresdb-peers");
    if (stored) {
      discoveredPeers = JSON.parse(stored);
    }
  }

  function loadPendingRequests() {
    // Load pending peer requests
    const stored = localStorage.getItem("pluresdb-requests");
    if (stored) {
      const requests = JSON.parse(stored);
      pendingRequests = requests.pending || [];
      sentRequests = requests.sent || [];
    }
  }

  async function searchPeers() {
    if (!searchQuery.trim()) return;

    isSearching = true;
    try {
      // Simulate peer search
      await new Promise((resolve) => setTimeout(resolve, 1000));

      // Mock search results
      searchResults = [
        {
          id: "peer_1",
          name: "Alice Developer",
          email: "alice@example.com",
          location: "San Francisco, CA",
          tags: ["developer", "rust", "p2p"],
          lastSeen: new Date(Date.now() - 300000),
          publicKey: "pk_alice_123456789",
        },
        {
          id: "peer_2",
          name: "Bob Researcher",
          email: "bob@university.edu",
          location: "Cambridge, UK",
          tags: ["researcher", "ai", "blockchain"],
          lastSeen: new Date(Date.now() - 600000),
          publicKey: "pk_bob_987654321",
        },
      ];

      toast.success(`Found ${searchResults.length} peers`);
    } catch (error) {
      toast.error("Search failed: " + error.message);
    } finally {
      isSearching = false;
    }
  }

  function sendPeerRequest(peer: any) {
    const request = {
      id: generateId(),
      from: identity.id,
      to: peer.id,
      message: `Hello ${peer.name}, I'd like to connect with you.`,
      timestamp: new Date(),
      status: "pending",
    };

    sentRequests.push(request);
    saveRequests();
    toast.success(`Peer request sent to ${peer.name}`);
  }

  function acceptPeerRequest(request: any) {
    request.status = "accepted";
    request.acceptedAt = new Date();

    // Add to discovered peers
    const peer = {
      id: request.from,
      name: "Unknown Peer",
      publicKey: "pk_unknown",
      lastSeen: new Date(),
      acceptedAt: new Date(),
    };
    discoveredPeers.push(peer);

    // Remove from pending
    pendingRequests = pendingRequests.filter((r) => r.id !== request.id);

    saveRequests();
    saveDiscoveredPeers();
    toast.success("Peer request accepted");
  }

  function rejectPeerRequest(request: any) {
    request.status = "rejected";
    request.rejectedAt = new Date();

    // Remove from pending
    pendingRequests = pendingRequests.filter((r) => r.id !== request.id);

    saveRequests();
    toast.info("Peer request rejected");
  }

  function saveRequests() {
    localStorage.setItem(
      "pluresdb-requests",
      JSON.stringify({
        pending: pendingRequests,
        sent: sentRequests,
      }),
    );
  }

  function saveDiscoveredPeers() {
    localStorage.setItem("pluresdb-peers", JSON.stringify(discoveredPeers));
  }

  function updateAcceptancePolicy() {
    if (currentPolicy === "custom") {
      // Custom policy is already in customPolicy
    } else {
      customPolicy = { ...acceptancePolicies[currentPolicy] };
    }
    toast.success("Acceptance policy updated");
  }

  function addTag() {
    const tag = prompt("Enter tag:");
    if (tag && !identity.tags.includes(tag)) {
      identity.tags.push(tag);
      saveIdentity();
    }
  }

  function removeTag(tag: string) {
    identity.tags = identity.tags.filter((t) => t !== tag);
    saveIdentity();
  }
</script>

<div class="identity-discovery">
  <div class="header">
    <h2>Identity & Discovery</h2>
    <p>Manage your identity and discover peers in the P2P network</p>
  </div>

  <div class="tabs">
    <button class="tab active">Identity</button>
    <button class="tab">Discovery</button>
    <button class="tab">Policies</button>
    <button class="tab">Requests</button>
  </div>

  <div class="tab-content">
    <!-- Identity Tab -->
    <div class="tab-panel active">
      <div class="identity-form">
        <div class="form-group">
          <label for="name">Name</label>
          <input id="name" type="text" bind:value={identity.name} placeholder="Your display name" />
        </div>

        <div class="form-group">
          <label for="email">Email</label>
          <input id="email" type="email" bind:value={identity.email} placeholder="your@email.com" />
        </div>

        <div class="form-group">
          <label for="phone">Phone</label>
          <input
            id="phone"
            type="tel"
            bind:value={identity.phone}
            placeholder="+1 (555) 123-4567"
          />
        </div>

        <div class="form-group">
          <label for="location">Location</label>
          <input
            id="location"
            type="text"
            bind:value={identity.location}
            placeholder="City, Country"
          />
        </div>

        <div class="form-group">
          <label for="bio">Bio</label>
          <textarea
            id="bio"
            bind:value={identity.bio}
            placeholder="Tell others about yourself..."
            rows="3"
          ></textarea>
        </div>

        <div class="form-group">
          <span class="form-label">Tags</span>
          <div class="tags">
            {#each identity.tags as tag}
              <span class="tag">
                {tag}
                <button type="button" on:click={() => removeTag(tag)}>×</button>
              </span>
            {/each}
            <button type="button" class="add-tag" on:click={addTag}>+ Add Tag</button>
          </div>
        </div>

        <div class="identity-info">
          <div class="info-item">
            <strong>ID:</strong>
            {identity.id}
          </div>
          <div class="info-item">
            <strong>Public Key:</strong>
            {identity.publicKey}
          </div>
          <div class="info-item">
            <strong>Created:</strong>
            {identity.createdAt.toLocaleDateString()}
          </div>
          <div class="info-item">
            <strong>Last Seen:</strong>
            {identity.lastSeen.toLocaleString()}
          </div>
        </div>

        <button class="btn btn-primary" on:click={saveIdentity}> Save Identity </button>
      </div>
    </div>

    <!-- Discovery Tab -->
    <div class="tab-panel">
      <div class="search-section">
        <div class="search-bar">
          <input
            type="text"
            bind:value={searchQuery}
            placeholder="Search for peers by name, email, or tags..."
            on:keydown={(e) => e.key === "Enter" && searchPeers()}
          />
          <button class="btn btn-primary" on:click={searchPeers} disabled={isSearching}>
            {isSearching ? "Searching..." : "Search"}
          </button>
        </div>

        {#if searchResults.length > 0}
          <div class="search-results">
            <h3>Search Results</h3>
            {#each searchResults as peer}
              <div class="peer-card">
                <div class="peer-info">
                  <h4>{peer.name}</h4>
                  <p>{peer.email}</p>
                  <p class="location">{peer.location}</p>
                  <div class="tags">
                    {#each peer.tags as tag}
                      <span class="tag">{tag}</span>
                    {/each}
                  </div>
                  <p class="last-seen">Last seen: {peer.lastSeen.toLocaleString()}</p>
                </div>
                <div class="peer-actions">
                  <button class="btn btn-success" on:click={() => sendPeerRequest(peer)}>
                    Send Request
                  </button>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>

      <div class="discovered-peers">
        <h3>Discovered Peers</h3>
        {#if discoveredPeers.length === 0}
          <p class="empty">No peers discovered yet</p>
        {:else}
          {#each discoveredPeers as peer}
            <div class="peer-card">
              <div class="peer-info">
                <h4>{peer.name}</h4>
                <p>ID: {peer.id}</p>
                <p>Public Key: {peer.publicKey}</p>
                <p class="last-seen">Last seen: {peer.lastSeen.toLocaleString()}</p>
              </div>
              <div class="peer-status">
                <span class="status connected">Connected</span>
              </div>
            </div>
          {/each}
        {/if}
      </div>
    </div>

    <!-- Policies Tab -->
    <div class="tab-panel">
      <div class="policy-section">
        <h3>Acceptance Policies</h3>
        <p>Configure what data you're willing to accept from peers</p>

        <div class="policy-selector">
          <label for="policy-type">Policy Type:</label>
          <select id="policy-type" bind:value={currentPolicy}>
            <option value="default">Default</option>
            <option value="laptop">Laptop</option>
            <option value="phone">Phone</option>
            <option value="server">Server</option>
            <option value="custom">Custom</option>
          </select>
        </div>

        <div class="policy-config">
          <div class="form-group">
            <label>
              <input type="checkbox" bind:checked={customPolicy.allowDataSharing} />
              Allow Data Sharing
            </label>
          </div>

          <div class="form-group">
            <label>
              <input type="checkbox" bind:checked={customPolicy.allowFileSharing} />
              Allow File Sharing
            </label>
          </div>

          <div class="form-group">
            <label>
              <input type="checkbox" bind:checked={customPolicy.allowLocationSharing} />
              Allow Location Sharing
            </label>
          </div>

          <div class="form-group">
            <label for="max-size">Max Data Size:</label>
            <select id="max-size" bind:value={customPolicy.maxDataSize}>
              <option value="1MB">1MB</option>
              <option value="10MB">10MB</option>
              <option value="50MB">50MB</option>
              <option value="100MB">100MB</option>
              <option value="1GB">1GB</option>
            </select>
          </div>

          <div class="form-group">
            <label for="allowed-types">Allowed Data Types:</label>
            <div class="checkbox-group">
              {#each ["text", "json", "image", "video", "audio", "database"] as type}
                <label>
                  <input type="checkbox" bind:group={customPolicy.allowedTypes} value={type} />
                  {type}
                </label>
              {/each}
            </div>
          </div>

          <div class="form-group">
            <label>
              <input type="checkbox" bind:checked={customPolicy.requireApproval} />
              Require Approval for New Peers
            </label>
          </div>
        </div>

        <button class="btn btn-primary" on:click={updateAcceptancePolicy}> Update Policy </button>
      </div>
    </div>

    <!-- Requests Tab -->
    <div class="tab-panel">
      <div class="requests-section">
        <h3>Pending Requests</h3>
        {#if pendingRequests.length === 0}
          <p class="empty">No pending requests</p>
        {:else}
          {#each pendingRequests as request}
            <div class="request-card">
              <div class="request-info">
                <h4>From: {request.from}</h4>
                <p>{request.message}</p>
                <p class="timestamp">Sent: {request.timestamp.toLocaleString()}</p>
              </div>
              <div class="request-actions">
                <button class="btn btn-success" on:click={() => acceptPeerRequest(request)}>
                  Accept
                </button>
                <button class="btn btn-danger" on:click={() => rejectPeerRequest(request)}>
                  Reject
                </button>
              </div>
            </div>
          {/each}
        {/if}

        <h3>Sent Requests</h3>
        {#if sentRequests.length === 0}
          <p class="empty">No sent requests</p>
        {:else}
          {#each sentRequests as request}
            <div class="request-card">
              <div class="request-info">
                <h4>To: {request.to}</h4>
                <p>{request.message}</p>
                <p class="timestamp">Sent: {request.timestamp.toLocaleString()}</p>
                <p class="status">Status: {request.status}</p>
              </div>
            </div>
          {/each}
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  .identity-discovery {
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

  .identity-form {
    max-width: 600px;
  }

  .form-group {
    margin-bottom: 1.5rem;
  }

  .form-group label,
  .form-label {
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

  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    margin-top: 0.5rem;
  }

  .tag {
    display: inline-flex;
    align-items: center;
    padding: 0.25rem 0.75rem;
    background: var(--primary);
    color: white;
    border-radius: 20px;
    font-size: 0.875rem;
  }

  .tag button {
    margin-left: 0.5rem;
    background: none;
    border: none;
    color: white;
    cursor: pointer;
    font-size: 1.2rem;
    line-height: 1;
  }

  .add-tag {
    padding: 0.25rem 0.75rem;
    background: var(--secondary);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 20px;
    cursor: pointer;
    font-size: 0.875rem;
  }

  .identity-info {
    background: var(--background);
    padding: 1rem;
    border-radius: 4px;
    margin: 1.5rem 0;
  }

  .info-item {
    margin-bottom: 0.5rem;
  }

  .info-item:last-child {
    margin-bottom: 0;
  }

  .search-bar {
    display: flex;
    gap: 1rem;
    margin-bottom: 2rem;
  }

  .search-bar input {
    flex: 1;
    padding: 0.75rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    font-size: 1rem;
  }

  .search-results,
  .discovered-peers {
    margin-bottom: 2rem;
  }

  .peer-card {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    margin-bottom: 1rem;
  }

  .peer-info h4 {
    margin: 0 0 0.5rem 0;
  }

  .peer-info p {
    margin: 0.25rem 0;
    color: var(--muted);
  }

  .peer-info .location {
    font-weight: 500;
  }

  .peer-info .tags {
    margin-top: 0.5rem;
  }

  .peer-info .tags .tag {
    background: var(--secondary);
    color: var(--text);
    font-size: 0.75rem;
  }

  .peer-actions {
    display: flex;
    gap: 0.5rem;
  }

  .peer-status .status {
    padding: 0.25rem 0.75rem;
    border-radius: 20px;
    font-size: 0.875rem;
    font-weight: 500;
  }

  .status.connected {
    background: var(--success);
    color: white;
  }

  .policy-section {
    max-width: 600px;
  }

  .policy-selector {
    margin-bottom: 2rem;
  }

  .policy-selector select {
    margin-left: 1rem;
    padding: 0.5rem;
    border: 1px solid var(--border);
    border-radius: 4px;
  }

  .policy-config {
    background: var(--background);
    padding: 1.5rem;
    border-radius: 4px;
    margin-bottom: 2rem;
  }

  .checkbox-group {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
    gap: 0.5rem;
    margin-top: 0.5rem;
  }

  .checkbox-group label {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-weight: normal;
  }

  .requests-section h3 {
    margin-top: 2rem;
  }

  .request-card {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    margin-bottom: 1rem;
  }

  .request-info h4 {
    margin: 0 0 0.5rem 0;
  }

  .request-info p {
    margin: 0.25rem 0;
    color: var(--muted);
  }

  .request-info .timestamp {
    font-size: 0.875rem;
  }

  .request-info .status {
    font-weight: 500;
  }

  .request-actions {
    display: flex;
    gap: 0.5rem;
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

  .btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .btn-primary {
    background: var(--primary);
    color: white;
  }

  .btn-primary:hover:not(:disabled) {
    background: var(--primary-dark);
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

  /* .btn-secondary styles removed because the class is not used */
</style>
