//! WebSocket handlers for real-time communication

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Arc,
};
use tokio::sync::{broadcast, Mutex};
use tracing::{debug, error, info, warn};

use crate::server::ApiState;

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebSocketMessage {
    /// Client subscribes to a channel
    Subscribe { channel: String },
    /// Client unsubscribes from a channel
    Unsubscribe { channel: String },
    /// Client sends a message to a channel
    Message { channel: String, data: serde_json::Value },
    /// Server sends a notification
    Notification { message: String, level: String },
    /// Server sends an error
    Error { message: String },
    /// Heartbeat/ping message
    Ping,
    /// Heartbeat/pong response
    Pong,
    /// Node update notification
    NodeUpdate { node_id: String, operation: String },
    /// Relationship update notification
    RelationshipUpdate { from: String, to: String, operation: String },
    /// Vector search result
    VectorSearchResult { query: String, results: Vec<serde_json::Value> },
    /// Graph change notification
    GraphChange { change_type: String, details: serde_json::Value },
}

/// WebSocket connection state
#[derive(Debug, Clone)]
pub struct WebSocketConnection {
    pub id: String,
    pub channels: Vec<String>,
    pub last_ping: std::time::Instant,
}

/// WebSocket manager
#[derive(Debug, Clone)]
pub struct WebSocketManager {
    /// Active connections
    connections: Arc<Mutex<HashMap<String, WebSocketConnection>>>,
    /// Channel subscriptions
    channels: Arc<Mutex<HashMap<String, Vec<String>>>>,
    /// Broadcast channels for each channel
    broadcasters: Arc<Mutex<HashMap<String, broadcast::Sender<WebSocketMessage>>>>,
}

impl WebSocketManager {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            channels: Arc::new(Mutex::new(HashMap::new())),
            broadcasters: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Add a new connection
    pub async fn add_connection(&self, id: String) -> WebSocketConnection {
        let connection = WebSocketConnection {
            id: id.clone(),
            channels: Vec::new(),
            last_ping: std::time::Instant::now(),
        };

        self.connections.lock().await.insert(id.clone(), connection.clone());
        info!("WebSocket connection added: {}", id);
        connection
    }

    /// Remove a connection
    pub async fn remove_connection(&self, id: &str) {
        let mut connections = self.connections.lock().await;
        if let Some(connection) = connections.remove(id) {
            // Remove from all channels
            for channel in &connection.channels {
                self.unsubscribe_from_channel(id, channel).await;
            }
            info!("WebSocket connection removed: {}", id);
        }
    }

    /// Subscribe to a channel
    pub async fn subscribe_to_channel(&self, connection_id: &str, channel: &str) {
        // Add to connection's channels
        {
            let mut connections = self.connections.lock().await;
            if let Some(connection) = connections.get_mut(connection_id) {
                if !connection.channels.contains(&channel.to_string()) {
                    connection.channels.push(channel.to_string());
                }
            }
        }

        // Add to channel's subscribers
        {
            let mut channels = self.channels.lock().await;
            let subscribers = channels.entry(channel.to_string()).or_insert_with(Vec::new);
            if !subscribers.contains(&connection_id.to_string()) {
                subscribers.push(connection_id.to_string());
            }
        }

        // Create broadcaster if it doesn't exist
        {
            let mut broadcasters = self.broadcasters.lock().await;
            if !broadcasters.contains_key(channel) {
                let (tx, _rx) = broadcast::channel(1000);
                broadcasters.insert(channel.to_string(), tx);
            }
        }

        debug!("Connection {} subscribed to channel {}", connection_id, channel);
    }

    /// Unsubscribe from a channel
    pub async fn unsubscribe_from_channel(&self, connection_id: &str, channel: &str) {
        // Remove from connection's channels
        {
            let mut connections = self.connections.lock().await;
            if let Some(connection) = connections.get_mut(connection_id) {
                connection.channels.retain(|c| c != channel);
            }
        }

        // Remove from channel's subscribers
        {
            let mut channels = self.channels.lock().await;
            if let Some(subscribers) = channels.get_mut(channel) {
                subscribers.retain(|id| id != connection_id);
            }
        }

        debug!("Connection {} unsubscribed from channel {}", connection_id, channel);
    }

    /// Get broadcaster for a channel
    pub async fn get_broadcaster(&self, channel: &str) -> Option<broadcast::Sender<WebSocketMessage>> {
        let broadcasters = self.broadcasters.lock().await;
        broadcasters.get(channel).cloned()
    }

    /// Broadcast message to a channel
    pub async fn broadcast_to_channel(&self, channel: &str, message: WebSocketMessage) {
        if let Some(tx) = self.get_broadcaster(channel).await {
            if let Err(e) = tx.send(message) {
                warn!("Failed to broadcast to channel {}: {}", channel, e);
            }
        }
    }

    /// Get connection info
    pub async fn get_connection(&self, id: &str) -> Option<WebSocketConnection> {
        let connections = self.connections.lock().await;
        connections.get(id).cloned()
    }

    /// Get all connections
    pub async fn get_all_connections(&self) -> Vec<WebSocketConnection> {
        let connections = self.connections.lock().await;
        connections.values().cloned().collect()
    }

    /// Get channel subscribers
    pub async fn get_channel_subscribers(&self, channel: &str) -> Vec<String> {
        let channels = self.channels.lock().await;
        channels.get(channel).cloned().unwrap_or_default()
    }

    /// Get statistics
    pub async fn get_stats(&self) -> serde_json::Value {
        let connections = self.connections.lock().await;
        let channels = self.channels.lock().await;
        let broadcasters = self.broadcasters.lock().await;

        serde_json::json!({
            "total_connections": connections.len(),
            "total_channels": channels.len(),
            "total_broadcasters": broadcasters.len(),
            "connections": connections.values().map(|c| serde_json::json!({
                "id": c.id,
                "channels": c.channels,
                "last_ping": c.last_ping.elapsed().as_secs()
            })).collect::<Vec<_>>(),
            "channels": channels.iter().map(|(name, subscribers)| serde_json::json!({
                "name": name,
                "subscriber_count": subscribers.len()
            })).collect::<Vec<_>>()
        })
    }
}

/// WebSocket handler
pub async fn websocket_handler(
    State(state): State<ApiState>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(|socket| handle_websocket(socket, state))
}

/// WebSocket channel handler
pub async fn websocket_channel_handler(
    State(state): State<ApiState>,
    Path(channel): Path<String>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| handle_websocket_channel(socket, channel, state))
}

/// Handle WebSocket connection
async fn handle_websocket(socket: WebSocket, state: ApiState) {
    let connection_id = uuid::Uuid::new_v4().to_string();
    let manager = Arc::new(WebSocketManager::new());
    
    info!("WebSocket connection established: {}", connection_id);
    
    let (mut sender, mut receiver) = socket.split();
    
    // Add connection to manager
    manager.add_connection(connection_id.clone()).await;
    
    // Handle incoming messages
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Ok(ws_message) = serde_json::from_str::<WebSocketMessage>(&text) {
                    handle_websocket_message(&connection_id, ws_message, &manager, &state).await;
                } else {
                    warn!("Invalid WebSocket message from {}: {}", connection_id, text);
                }
            }
            Ok(Message::Binary(data)) => {
                debug!("Received binary message from {}: {} bytes", connection_id, data.len());
            }
            Ok(Message::Ping(data)) => {
                if let Err(e) = sender.send(Message::Pong(data)).await {
                    error!("Failed to send pong to {}: {}", connection_id, e);
                    break;
                }
            }
            Ok(Message::Pong(_)) => {
                // Update last ping time
                if let Some(connection) = manager.get_connection(&connection_id).await {
                    // Would need to update last_ping in the connection
                }
            }
            Ok(Message::Close(_)) => {
                info!("WebSocket connection closed: {}", connection_id);
                break;
            }
            Err(e) => {
                error!("WebSocket error for {}: {}", connection_id, e);
                break;
            }
        }
    }
    
    // Clean up connection
    manager.remove_connection(&connection_id).await;
    info!("WebSocket connection cleaned up: {}", connection_id);
}

/// Handle WebSocket channel connection
async fn handle_websocket_channel(socket: WebSocket, channel: String, state: ApiState) {
    let connection_id = uuid::Uuid::new_v4().to_string();
    let manager = Arc::new(WebSocketManager::new());
    
    info!("WebSocket channel connection established: {} on channel {}", connection_id, channel);
    
    let (mut sender, mut receiver) = socket.split();
    
    // Add connection and subscribe to channel
    manager.add_connection(connection_id.clone()).await;
    manager.subscribe_to_channel(&connection_id, &channel).await;
    
    // Start listening to channel broadcasts
    let mut rx = if let Some(tx) = manager.get_broadcaster(&channel).await {
        tx.subscribe()
    } else {
        return;
    };
    
    // Spawn task to handle broadcasts
    let channel_clone = channel.clone();
    let manager_clone = manager.clone();
    let sender_clone = sender.clone();
    tokio::spawn(async move {
        while let Ok(message) = rx.recv().await {
            if let Err(e) = sender_clone.send(Message::Text(serde_json::to_string(&message).unwrap())).await {
                error!("Failed to send broadcast to {}: {}", connection_id, e);
                break;
            }
        }
    });
    
    // Handle incoming messages
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Ok(ws_message) = serde_json::from_str::<WebSocketMessage>(&text) {
                    handle_websocket_message(&connection_id, ws_message, &manager, &state).await;
                } else {
                    warn!("Invalid WebSocket message from {}: {}", connection_id, text);
                }
            }
            Ok(Message::Binary(data)) => {
                debug!("Received binary message from {}: {} bytes", connection_id, data.len());
            }
            Ok(Message::Ping(data)) => {
                if let Err(e) = sender.send(Message::Pong(data)).await {
                    error!("Failed to send pong to {}: {}", connection_id, e);
                    break;
                }
            }
            Ok(Message::Pong(_)) => {
                // Update last ping time
            }
            Ok(Message::Close(_)) => {
                info!("WebSocket channel connection closed: {} on channel {}", connection_id, channel);
                break;
            }
            Err(e) => {
                error!("WebSocket error for {} on channel {}: {}", connection_id, channel, e);
                break;
            }
        }
    }
    
    // Clean up connection
    manager.remove_connection(&connection_id).await;
    info!("WebSocket channel connection cleaned up: {} from channel {}", connection_id, channel);
}

/// Handle WebSocket message
async fn handle_websocket_message(
    connection_id: &str,
    message: WebSocketMessage,
    manager: &WebSocketManager,
    state: &ApiState,
) {
    match message {
        WebSocketMessage::Subscribe { channel } => {
            manager.subscribe_to_channel(connection_id, &channel).await;
            
            // Send confirmation
            let confirmation = WebSocketMessage::Notification {
                message: format!("Subscribed to channel: {}", channel),
                level: "info".to_string(),
            };
            manager.broadcast_to_channel(&channel, confirmation).await;
        }
        
        WebSocketMessage::Unsubscribe { channel } => {
            manager.unsubscribe_from_channel(connection_id, &channel).await;
            
            // Send confirmation
            let confirmation = WebSocketMessage::Notification {
                message: format!("Unsubscribed from channel: {}", channel),
                level: "info".to_string(),
            };
            manager.broadcast_to_channel(&channel, confirmation).await;
        }
        
        WebSocketMessage::Message { channel, data } => {
            // Broadcast message to channel
            let broadcast_msg = WebSocketMessage::Message {
                channel: channel.clone(),
                data,
            };
            manager.broadcast_to_channel(&channel, broadcast_msg).await;
        }
        
        WebSocketMessage::Ping => {
            // Send pong response
            let pong = WebSocketMessage::Pong;
            // Would need to send directly to connection
        }
        
        WebSocketMessage::Pong => {
            // Update last ping time
            debug!("Received pong from {}", connection_id);
        }
        
        _ => {
            warn!("Unhandled WebSocket message from {}: {:?}", connection_id, message);
        }
    }
}

/// Broadcast node update
pub async fn broadcast_node_update(
    manager: &WebSocketManager,
    node_id: &str,
    operation: &str,
    data: serde_json::Value,
) {
    let message = WebSocketMessage::NodeUpdate {
        node_id: node_id.to_string(),
        operation: operation.to_string(),
    };
    
    // Broadcast to all channels (or specific channels)
    manager.broadcast_to_channel("nodes", message).await;
}

/// Broadcast relationship update
pub async fn broadcast_relationship_update(
    manager: &WebSocketManager,
    from: &str,
    to: &str,
    operation: &str,
) {
    let message = WebSocketMessage::RelationshipUpdate {
        from: from.to_string(),
        to: to.to_string(),
        operation: operation.to_string(),
    };
    
    // Broadcast to all channels (or specific channels)
    manager.broadcast_to_channel("relationships", message).await;
}

/// Broadcast vector search result
pub async fn broadcast_vector_search_result(
    manager: &WebSocketManager,
    query: &str,
    results: Vec<serde_json::Value>,
) {
    let message = WebSocketMessage::VectorSearchResult {
        query: query.to_string(),
        results,
    };
    
    manager.broadcast_to_channel("vector_search", message).await;
}

/// Broadcast graph change
pub async fn broadcast_graph_change(
    manager: &WebSocketManager,
    change_type: &str,
    details: serde_json::Value,
) {
    let message = WebSocketMessage::GraphChange {
        change_type: change_type.to_string(),
        details,
    };
    
    manager.broadcast_to_channel("graph", message).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_websocket_manager() {
        let manager = WebSocketManager::new();
        
        // Add connection
        let connection = manager.add_connection("test-connection".to_string()).await;
        assert_eq!(connection.id, "test-connection");
        
        // Subscribe to channel
        manager.subscribe_to_channel("test-connection", "test-channel").await;
        
        // Check subscription
        let subscribers = manager.get_channel_subscribers("test-channel").await;
        assert!(subscribers.contains(&"test-connection".to_string()));
        
        // Remove connection
        manager.remove_connection("test-connection").await;
        
        // Check removal
        let subscribers = manager.get_channel_subscribers("test-channel").await;
        assert!(!subscribers.contains(&"test-connection".to_string()));
    }

    #[test]
    fn test_websocket_message_serialization() {
        let message = WebSocketMessage::Subscribe {
            channel: "test".to_string(),
        };
        
        let json = serde_json::to_string(&message).unwrap();
        let deserialized: WebSocketMessage = serde_json::from_str(&json).unwrap();
        
        match deserialized {
            WebSocketMessage::Subscribe { channel } => {
                assert_eq!(channel, "test");
            }
            _ => panic!("Expected Subscribe message"),
        }
    }
}


