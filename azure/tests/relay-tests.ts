/**
 * Azure Relay Integration Tests
 * 
 * Tests P2P relay functionality across multiple PluresDB nodes
 * deployed in Azure Container Instances.
 */

import { assertEquals, assertExists } from "https://deno.land/std@0.208.0/assert/mod.ts";

interface NodeInfo {
  name: string;
  ipAddress: string;
  port: number;
  apiPort: number;
}

interface TestContext {
  environment: string;
  nodes: NodeInfo[];
}

/**
 * Get node information from Azure deployment
 */
async function getDeployedNodes(environment: string): Promise<NodeInfo[]> {
  // In a real scenario, this would query Azure API or use deployment outputs
  // For now, we'll use environment variables or a config file
  
  const nodeCount = parseInt(Deno.env.get("AZURE_NODE_COUNT") || "3");
  const baseIP = Deno.env.get("AZURE_NODE_BASE_IP") || "20.0.0";
  
  const nodes: NodeInfo[] = [];
  for (let i = 0; i < nodeCount; i++) {
    nodes.push({
      name: `pluresdb-${environment}-node-${i}`,
      ipAddress: `${baseIP}.${i + 10}`,
      port: 34567,
      apiPort: 34568,
    });
  }
  
  return nodes;
}

/**
 * Check if a node is healthy
 */
async function checkNodeHealth(node: NodeInfo): Promise<boolean> {
  try {
    const response = await fetch(`http://${node.ipAddress}:${node.apiPort}/health`, {
      signal: AbortSignal.timeout(5000),
    });
    return response.ok;
  } catch {
    return false;
  }
}

/**
 * Get peers list from a node
 */
async function getNodePeers(node: NodeInfo): Promise<string[]> {
  try {
    const response = await fetch(`http://${node.ipAddress}:${node.apiPort}/peers`, {
      signal: AbortSignal.timeout(5000),
    });
    
    if (!response.ok) {
      return [];
    }
    
    const data = await response.json();
    return data.peers || [];
  } catch {
    return [];
  }
}

/**
 * Write data to a node
 */
async function writeData(node: NodeInfo, key: string, value: unknown): Promise<boolean> {
  try {
    const response = await fetch(`http://${node.ipAddress}:${node.apiPort}/data`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ key, value }),
      signal: AbortSignal.timeout(5000),
    });
    return response.ok;
  } catch {
    return false;
  }
}

/**
 * Read data from a node
 */
async function readData(node: NodeInfo, key: string): Promise<unknown> {
  try {
    const response = await fetch(`http://${node.ipAddress}:${node.apiPort}/data/${key}`, {
      signal: AbortSignal.timeout(5000),
    });
    
    if (!response.ok) {
      return null;
    }
    
    const data = await response.json();
    return data.value;
  } catch {
    return null;
  }
}

/**
 * Wait for data to propagate across nodes
 */
async function waitForPropagation(
  nodes: NodeInfo[],
  key: string,
  expectedValue: unknown,
  timeoutMs: number = 10000,
): Promise<boolean> {
  const startTime = Date.now();
  
  while (Date.now() - startTime < timeoutMs) {
    let allMatch = true;
    
    for (const node of nodes) {
      const value = await readData(node, key);
      if (JSON.stringify(value) !== JSON.stringify(expectedValue)) {
        allMatch = false;
        break;
      }
    }
    
    if (allMatch) {
      return true;
    }
    
    await new Promise(resolve => setTimeout(resolve, 500));
  }
  
  return false;
}

// Test Suite

Deno.test({
  name: "Azure Relay - Node Health Check",
  async fn() {
    const nodes = await getDeployedNodes("test");
    
    for (const node of nodes) {
      const healthy = await checkNodeHealth(node);
      assertEquals(
        healthy,
        true,
        `Node ${node.name} should be healthy`,
      );
    }
  },
  ignore: Deno.env.get("SKIP_AZURE_TESTS") === "true",
});

Deno.test({
  name: "Azure Relay - Node Discovery",
  async fn() {
    const nodes = await getDeployedNodes("test");
    assertEquals(nodes.length >= 2, true, "Should have at least 2 nodes for P2P testing");
    
    // Wait for nodes to discover each other
    await new Promise(resolve => setTimeout(resolve, 30000)); // 30 seconds
    
    for (const node of nodes) {
      const peers = await getNodePeers(node);
      assertEquals(
        peers.length >= 1,
        true,
        `Node ${node.name} should have discovered at least one peer`,
      );
    }
  },
  ignore: Deno.env.get("SKIP_AZURE_TESTS") === "true",
});

Deno.test({
  name: "Azure Relay - Data Propagation",
  async fn() {
    const nodes = await getDeployedNodes("test");
    assertEquals(nodes.length >= 2, true, "Should have at least 2 nodes for P2P testing");
    
    const testKey = `test-${Date.now()}`;
    const testValue = { message: "Hello from PluresDB", timestamp: Date.now() };
    
    // Write to first node
    const writeSuccess = await writeData(nodes[0], testKey, testValue);
    assertEquals(writeSuccess, true, "Should successfully write data to node 0");
    
    // Wait for propagation to all nodes
    const propagated = await waitForPropagation(nodes, testKey, testValue, 10000);
    assertEquals(propagated, true, "Data should propagate to all nodes within 10 seconds");
  },
  ignore: Deno.env.get("SKIP_AZURE_TESTS") === "true",
});

Deno.test({
  name: "Azure Relay - Multi-Node Write Consistency",
  async fn() {
    const nodes = await getDeployedNodes("test");
    assertEquals(nodes.length >= 2, true, "Should have at least 2 nodes for P2P testing");
    
    const testKey = `consistency-test-${Date.now()}`;
    
    // Write different values from different nodes
    const writes = nodes.map(async (node, index) => {
      const value = { nodeIndex: index, timestamp: Date.now() };
      return await writeData(node, `${testKey}-${index}`, value);
    });
    
    const results = await Promise.all(writes);
    assertEquals(
      results.every(r => r === true),
      true,
      "All writes should succeed",
    );
    
    // Give time for CRDT resolution
    await new Promise(resolve => setTimeout(resolve, 5000));
    
    // Verify all nodes have all keys
    for (let i = 0; i < nodes.length; i++) {
      const key = `${testKey}-${i}`;
      
      for (const node of nodes) {
        const value = await readData(node, key);
        assertExists(value, `All nodes should have key ${key}`);
      }
    }
  },
  ignore: Deno.env.get("SKIP_AZURE_TESTS") === "true",
});

Deno.test({
  name: "Azure Relay - Latency Measurement",
  async fn() {
    const nodes = await getDeployedNodes("test");
    assertEquals(nodes.length >= 2, true, "Should have at least 2 nodes for latency testing");
    
    const testKey = `latency-test-${Date.now()}`;
    const testValue = { data: "latency test" };
    
    // Write to first node and measure propagation time
    const startTime = Date.now();
    await writeData(nodes[0], testKey, testValue);
    
    // Poll second node until data appears
    let latency = 0;
    const maxWait = 5000; // 5 seconds
    
    while (latency < maxWait) {
      const value = await readData(nodes[1], testKey);
      if (value !== null) {
        latency = Date.now() - startTime;
        break;
      }
      await new Promise(resolve => setTimeout(resolve, 100));
      latency = Date.now() - startTime;
    }
    
    console.log(`Propagation latency: ${latency}ms`);
    
    // Assert reasonable latency (< 5 seconds for Azure same-region)
    assertEquals(latency < 5000, true, "Propagation should complete within 5 seconds");
  },
  ignore: Deno.env.get("SKIP_AZURE_TESTS") === "true",
});

Deno.test({
  name: "Azure Relay - Throughput Test",
  async fn() {
    const nodes = await getDeployedNodes("test");
    assertEquals(nodes.length >= 1, true, "Should have at least 1 node for throughput testing");
    
    const node = nodes[0];
    const numWrites = 100;
    const startTime = Date.now();
    
    // Perform multiple writes
    const writes = [];
    for (let i = 0; i < numWrites; i++) {
      writes.push(
        writeData(node, `throughput-test-${i}`, { index: i, data: "test" }),
      );
    }
    
    const results = await Promise.all(writes);
    const duration = Date.now() - startTime;
    const throughput = (numWrites / duration) * 1000; // ops/sec
    
    console.log(`Throughput: ${throughput.toFixed(2)} writes/sec`);
    console.log(`Average latency: ${(duration / numWrites).toFixed(2)}ms per write`);
    
    assertEquals(
      results.every(r => r === true),
      true,
      "All writes should succeed",
    );
    assertEquals(throughput > 10, true, "Throughput should be at least 10 writes/sec");
  },
  ignore: Deno.env.get("SKIP_AZURE_TESTS") === "true",
});
