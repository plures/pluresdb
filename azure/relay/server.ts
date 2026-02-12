/**
 * PluresDB Azure WSS Relay Server
 * 
 * Corporate-safe WebSocket relay server that:
 * - Runs on port 443 (looks like HTTPS to firewalls)
 * - Matches peers by topic hash
 * - Pipes encrypted bytes between peers
 * - Stateless and horizontally scalable
 */

import { WebSocketServer, WebSocket } from "ws";

interface PeerConnection {
  id: string;
  topic: string;
  socket: WebSocket;
  connectedAt: Date;
}

interface RelayMessage {
  type: "join" | "offer" | "answer" | "data" | "peer-joined" | "peer-left" | "error";
  peerId?: string;
  topic?: string;
  data?: string;
  payload?: unknown;
}

// Topic-based peer matching
const topics = new Map<string, Set<PeerConnection>>();

// Peer tracking
const peers = new Map<WebSocket, PeerConnection>();

/**
 * Add peer to topic
 */
function joinTopic(peer: PeerConnection): void {
  if (!topics.has(peer.topic)) {
    topics.set(peer.topic, new Set());
  }

  const topicPeers = topics.get(peer.topic)!;
  topicPeers.add(peer);
  peers.set(peer.socket, peer);

  console.log(`[Relay] Peer ${peer.id} joined topic ${peer.topic} (${topicPeers.size} peers in topic)`);

  // Notify existing peers in the topic
  for (const otherPeer of topicPeers) {
    if (otherPeer !== peer && otherPeer.socket.readyState === WebSocket.OPEN) {
      const msg: RelayMessage = {
        type: "peer-joined",
        peerId: peer.id,
        topic: peer.topic,
      };
      otherPeer.socket.send(JSON.stringify(msg));
    }
  }

  // Notify the new peer about existing peers
  if (topicPeers.size > 1) {
    const msg: RelayMessage = {
      type: "peer-joined",
      peerId: Array.from(topicPeers).find(p => p !== peer)?.id,
      topic: peer.topic,
    };
    peer.socket.send(JSON.stringify(msg));
  }
}

/**
 * Remove peer from topic
 */
function leaveTopic(peer: PeerConnection): void {
  const topicPeers = topics.get(peer.topic);
  if (topicPeers) {
    topicPeers.delete(peer);

    if (topicPeers.size === 0) {
      topics.delete(peer.topic);
    } else {
      // Notify remaining peers
      for (const otherPeer of topicPeers) {
        if (otherPeer.socket.readyState === WebSocket.OPEN) {
          const msg: RelayMessage = {
            type: "peer-left",
            peerId: peer.id,
            topic: peer.topic,
          };
          otherPeer.socket.send(JSON.stringify(msg));
        }
      }
    }
  }

  peers.delete(peer.socket);
  console.log(`[Relay] Peer ${peer.id} left topic ${peer.topic}`);
}

/**
 * Relay data message to all other peers in the topic
 */
function relayData(from: PeerConnection, data: string): void {
  const topicPeers = topics.get(from.topic);
  if (!topicPeers) return;

  const msg: RelayMessage = {
    type: "data",
    peerId: from.id,
    data,
  };

  const msgStr = JSON.stringify(msg);
  let sentCount = 0;

  for (const peer of topicPeers) {
    if (peer !== from && peer.socket.readyState === WebSocket.OPEN) {
      peer.socket.send(msgStr);
      sentCount++;
    }
  }

  console.log(`[Relay] Relayed data from ${from.id} to ${sentCount} peers in topic ${from.topic}`);
}

/**
 * Start the relay server
 */
function startRelayServer(port: number): void {
  const wss = new WebSocketServer({ port });

  wss.on("listening", () => {
    console.log(`[Relay] PluresDB WSS Relay Server listening on port ${port}`);
    console.log(`[Relay] Endpoint: wss://localhost:${port}`);
  });

  wss.on("connection", (socket: WebSocket) => {
    let peer: PeerConnection | undefined;

    socket.on("message", (raw: Buffer) => {
      try {
        const msg = JSON.parse(raw.toString()) as RelayMessage;

        switch (msg.type) {
          case "join":
            if (!msg.topic) {
              const errorMsg: RelayMessage = {
                type: "error",
                payload: "Topic is required for join",
              };
              socket.send(JSON.stringify(errorMsg));
              return;
            }

            peer = {
              id: msg.peerId || `peer-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
              topic: msg.topic,
              socket,
              connectedAt: new Date(),
            };

            joinTopic(peer);
            break;

          case "data":
            if (peer && msg.data) {
              relayData(peer, msg.data);
            }
            break;

          default:
            console.warn(`[Relay] Unknown message type: ${msg.type}`);
        }
      } catch (error) {
        console.error("[Relay] Message parsing error:", error);
      }
    });

    socket.on("close", () => {
      if (peer) {
        leaveTopic(peer);
      }
    });

    socket.on("error", (error: Error) => {
      console.error("[Relay] WebSocket error:", error);
      if (peer) {
        leaveTopic(peer);
      }
    });
  });

  wss.on("error", (error: Error) => {
    console.error("[Relay] Server error:", error);
  });

  // Periodic cleanup of dead connections
  setInterval(() => {
    let cleanedCount = 0;

    for (const [socket, peer] of peers.entries()) {
      if (socket.readyState !== WebSocket.OPEN) {
        leaveTopic(peer);
        cleanedCount++;
      }
    }

    if (cleanedCount > 0) {
      console.log(`[Relay] Cleaned up ${cleanedCount} dead connections`);
    }
  }, 30000); // Every 30 seconds

  // Log stats every 5 minutes
  setInterval(() => {
    console.log(`[Relay] Stats: ${topics.size} topics, ${peers.size} total peers`);
    for (const [topic, topicPeers] of topics.entries()) {
      console.log(`[Relay]   - ${topic}: ${topicPeers.size} peers`);
    }
  }, 300000);
}

// Start server
const port = parseInt(process.env.PORT || "443");
startRelayServer(port);
