/**
 * Auto Transport with Fallback Chain
 * 
 * Automatically selects the best transport with fallback:
 * 1. Try direct (Hyperswarm) first
 * 2. If UDP blocked → fall back to Azure relay
 * 3. If Azure unavailable → fall back to Vercel relay
 */

import type { SyncConnection, SyncTransport, TransportConfig } from "../transport.ts";
import { AzureRelayTransport } from "./azure-relay.ts";
import { DirectTransport } from "./direct.ts";

/**
 * Auto transport with intelligent fallback
 */
export class AutoTransport implements SyncTransport {
  readonly name = "auto";

  private config: TransportConfig;
  private currentTransport?: SyncTransport;
  private transportOrder: SyncTransport[] = [];

  constructor(config: TransportConfig) {
    this.config = config;

    // Build transport chain based on configuration
    // Try direct first if available
    this.transportOrder.push(new DirectTransport());

    // Azure relay as primary relay
    if (config.azureRelayUrl) {
      this.transportOrder.push(
        new AzureRelayTransport({
          relayUrl: config.azureRelayUrl,
        }),
      );
    }

    // Vercel relay as backup
    if (config.vercelRelayUrl) {
      this.transportOrder.push(
        new AzureRelayTransport({
          relayUrl: config.vercelRelayUrl,
        }),
      );
    }
  }

  async connect(peerId: string): Promise<SyncConnection> {
    const errors: Error[] = [];

    // Try each transport in order
    for (const transport of this.transportOrder) {
      try {
        console.log(`[AutoTransport] Trying ${transport.name} transport...`);

        const timeoutMs = this.config.connectionTimeoutMs || 30000;
        const connection = await Promise.race([
          transport.connect(peerId),
          new Promise<never>((_, reject) =>
            setTimeout(
              () => reject(new Error(`${transport.name} connection timeout`)),
              timeoutMs,
            )
          ),
        ]);

        console.log(`[AutoTransport] Connected via ${transport.name}`);
        this.currentTransport = transport;
        return connection;
      } catch (error) {
        console.warn(
          `[AutoTransport] ${transport.name} failed:`,
          error instanceof Error ? error.message : error,
        );
        errors.push(
          error instanceof Error
            ? error
            : new Error(`${transport.name} connection failed`),
        );
        // Continue to next transport
      }
    }

    // If all transports failed, throw an error with details
    throw new Error(
      `All transports failed:\n${errors.map((e) => `  - ${e.message}`).join("\n")}`,
    );
  }

  async listen(onConnection: (conn: SyncConnection) => void): Promise<void> {
    const errors: Error[] = [];

    // Try to listen on each transport in order
    for (const transport of this.transportOrder) {
      try {
        console.log(`[AutoTransport] Listening on ${transport.name} transport...`);
        await transport.listen(onConnection);
        console.log(`[AutoTransport] Listening on ${transport.name}`);
        this.currentTransport = transport;
        return;
      } catch (error) {
        console.warn(
          `[AutoTransport] ${transport.name} listen failed:`,
          error instanceof Error ? error.message : error,
        );
        errors.push(
          error instanceof Error
            ? error
            : new Error(`${transport.name} listen failed`),
        );
        // Continue to next transport
      }
    }

    // If all transports failed, throw an error with details
    throw new Error(
      `All transports failed to listen:\n${errors.map((e) => `  - ${e.message}`).join("\n")}`,
    );
  }

  async close(): Promise<void> {
    // Close all transports
    const closePromises = this.transportOrder.map((transport) => transport.close());
    await Promise.all(closePromises);
    this.currentTransport = undefined;
  }
}

/**
 * Create transport based on configuration
 */
export function createTransport(config: TransportConfig): SyncTransport {
  switch (config.mode) {
    case "azure-relay":
      if (!config.azureRelayUrl) {
        throw new Error("Azure relay URL is required for azure-relay mode");
      }
      return new AzureRelayTransport({
        relayUrl: config.azureRelayUrl,
      });

    case "vercel-relay":
      if (!config.vercelRelayUrl) {
        throw new Error("Vercel relay URL is required for vercel-relay mode");
      }
      return new AzureRelayTransport({
        relayUrl: config.vercelRelayUrl,
      });

    case "direct":
      return new DirectTransport();

    case "auto":
    default:
      return new AutoTransport(config);
  }
}
