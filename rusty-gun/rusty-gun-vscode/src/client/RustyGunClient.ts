import * as vscode from "vscode";
import axios, { AxiosInstance, AxiosResponse } from "axios";
import { ConfigurationManager } from "../config/ConfigurationManager";
import { NotificationManager } from "../ui/NotificationManager";

export interface Node {
  id: string;
  data: any;
  metadata: any;
  tags: string[];
  created_at: string;
  updated_at: string;
}

export interface Relationship {
  id: string;
  from: string;
  to: string;
  relation_type: string;
  metadata: any;
  created_at: string;
}

export interface VectorSearchResult {
  id: string;
  score: number;
  metadata: any;
  text_hash: string;
}

export interface GraphStats {
  node_count: number;
  relationship_count: number;
  storage_size: number;
  index_count: number;
  last_updated: string;
}

export interface VectorStats {
  vector_count: number;
  dimensions: number;
  index_size: number;
  last_updated: string;
  cache_size: number;
  cache_hits: number;
  cache_misses: number;
}

export interface ServerStatus {
  status: string;
  version: string;
  uptime: number;
  services: {
    storage: string;
    vector_search: string;
    network: string;
    api: string;
  };
  timestamp: string;
}

export class RustyGunClient {
  private httpClient: AxiosInstance;
  private wsClient: WebSocket | null = null;
  private isConnectedFlag = false;
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 5;
  private reconnectDelay = 3000;

  constructor(
    private configManager: ConfigurationManager,
    private notificationManager: NotificationManager,
  ) {
    const serverUrl = configManager.get("serverUrl", "http://localhost:34569");
    this.httpClient = axios.create({
      baseURL: serverUrl,
      timeout: 10000,
      headers: {
        "Content-Type": "application/json",
      },
    });

    // Add request interceptor for error handling
    this.httpClient.interceptors.response.use(
      (response) => response,
      (error) => {
        if (error.code === "ECONNREFUSED" || error.code === "ENOTFOUND") {
          this.isConnectedFlag = false;
          this.notificationManager.showError(
            "Failed to connect to Rusty Gun server",
          );
        }
        return Promise.reject(error);
      },
    );
  }

  async connect(): Promise<boolean> {
    try {
      const response = await this.httpClient.get("/health");
      if (response.data.success) {
        this.isConnectedFlag = true;
        this.reconnectAttempts = 0;
        this.notificationManager.showInfo("Connected to Rusty Gun server");
        this.connectWebSocket();
        return true;
      } else {
        throw new Error("Server health check failed");
      }
    } catch (error) {
      this.isConnectedFlag = false;
      this.notificationManager.showError(
        `Failed to connect to Rusty Gun server: ${error}`,
      );
      return false;
    }
  }

  async disconnect(): Promise<void> {
    this.isConnectedFlag = false;
    if (this.wsClient) {
      this.wsClient.close();
      this.wsClient = null;
    }
    this.notificationManager.showInfo("Disconnected from Rusty Gun server");
  }

  isConnected(): boolean {
    return this.isConnectedFlag;
  }

  private connectWebSocket(): void {
    try {
      const wsUrl =
        this.configManager.get("serverUrl", "http://localhost:34569")
          .replace("http://", "ws://")
          .replace("https://", "wss://") + "/ws";

      this.wsClient = new WebSocket(wsUrl);

      this.wsClient.onopen = () => {
        console.log("WebSocket connected");
        this.reconnectAttempts = 0;
      };

      this.wsClient.onmessage = (event) => {
        try {
          const message = JSON.parse(event.data);
          this.handleWebSocketMessage(message);
        } catch (error) {
          console.error("Failed to parse WebSocket message:", error);
        }
      };

      this.wsClient.onclose = () => {
        console.log("WebSocket disconnected");
        this.attemptReconnect();
      };

      this.wsClient.onerror = (error) => {
        console.error("WebSocket error:", error);
      };
    } catch (error) {
      console.error("Failed to connect WebSocket:", error);
    }
  }

  private attemptReconnect(): void {
    if (this.reconnectAttempts < this.maxReconnectAttempts) {
      this.reconnectAttempts++;
      setTimeout(() => {
        if (this.isConnectedFlag) {
          this.connectWebSocket();
        }
      }, this.reconnectDelay * this.reconnectAttempts);
    }
  }

  private handleWebSocketMessage(message: any): void {
    // Handle real-time updates
    switch (message.type) {
      case "node_created":
      case "node_updated":
      case "node_deleted":
        vscode.commands.executeCommand("rusty-gun.refreshExplorer");
        break;
      case "relationship_created":
      case "relationship_updated":
      case "relationship_deleted":
        vscode.commands.executeCommand("rusty-gun.refreshExplorer");
        break;
      case "vector_added":
      case "vector_updated":
      case "vector_deleted":
        vscode.commands.executeCommand("rusty-gun.refreshExplorer");
        break;
    }
  }

  updateConfiguration(): void {
    const serverUrl = this.configManager.get(
      "serverUrl",
      "http://localhost:34569",
    );
    this.httpClient.defaults.baseURL = serverUrl;
  }

  // Node operations
  async getNodes(limit = 100, offset = 0): Promise<Node[]> {
    const response = await this.httpClient.get(
      `/nodes?limit=${limit}&offset=${offset}`,
    );
    return response.data.data || [];
  }

  async getNode(id: string): Promise<Node | null> {
    try {
      const response = await this.httpClient.get(`/nodes/${id}`);
      return response.data.data || null;
    } catch (error) {
      return null;
    }
  }

  async createNode(nodeData: Partial<Node>): Promise<Node> {
    const response = await this.httpClient.post("/nodes", nodeData);
    return response.data.data;
  }

  async updateNode(id: string, nodeData: Partial<Node>): Promise<Node> {
    const response = await this.httpClient.put(`/nodes/${id}`, nodeData);
    return response.data.data;
  }

  async deleteNode(id: string): Promise<void> {
    await this.httpClient.delete(`/nodes/${id}`);
  }

  async searchNodes(query: string, limit = 10): Promise<Node[]> {
    const response = await this.httpClient.post("/nodes/search", {
      query,
      limit,
    });
    return response.data.data || [];
  }

  // Relationship operations
  async getRelationships(nodeId?: string): Promise<Relationship[]> {
    const url = nodeId ? `/relationships?node_id=${nodeId}` : "/relationships";
    const response = await this.httpClient.get(url);
    return response.data.data || [];
  }

  async createRelationship(
    relationshipData: Partial<Relationship>,
  ): Promise<Relationship> {
    const response = await this.httpClient.post(
      "/relationships",
      relationshipData,
    );
    return response.data.data;
  }

  async updateRelationship(
    id: string,
    relationshipData: Partial<Relationship>,
  ): Promise<Relationship> {
    const response = await this.httpClient.put(
      `/relationships/${id}`,
      relationshipData,
    );
    return response.data.data;
  }

  async deleteRelationship(id: string): Promise<void> {
    await this.httpClient.delete(`/relationships/${id}`);
  }

  // Vector search operations
  async searchVectors(
    query: string,
    limit = 5,
    threshold = 0.3,
  ): Promise<VectorSearchResult[]> {
    const response = await this.httpClient.post("/vector/search/text", {
      query,
      limit,
      threshold,
    });
    return response.data.data?.results || [];
  }

  async addVectorText(
    id: string,
    text: string,
    metadata: any = {},
  ): Promise<void> {
    await this.httpClient.post("/vector/text", { id, text, metadata });
  }

  async getVectorStats(): Promise<VectorStats> {
    const response = await this.httpClient.get("/vector/stats");
    return response.data.data;
  }

  // SQL operations
  async executeSQL(query: string, params: any[] = []): Promise<any> {
    const response = await this.httpClient.post("/sql/query", {
      query,
      params,
    });
    return response.data.data;
  }

  // Graph operations
  async getGraphStats(): Promise<GraphStats> {
    const response = await this.httpClient.get("/graph/stats");
    return response.data.data;
  }

  async findPath(from: string, to: string): Promise<string[]> {
    const response = await this.httpClient.get(`/graph/path/${from}/${to}`);
    return response.data.data?.path || [];
  }

  // Server operations
  async getServerStatus(): Promise<ServerStatus> {
    const response = await this.httpClient.get("/health");
    return response.data.data;
  }

  // Export/Import operations
  async exportGraph(): Promise<any> {
    const response = await this.httpClient.get("/graph/export");
    return response.data.data;
  }

  async importGraph(data: any): Promise<void> {
    await this.httpClient.post("/graph/import", data);
  }
}
