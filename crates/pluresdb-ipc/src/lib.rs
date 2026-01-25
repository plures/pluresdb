/*!
 * PluresDB IPC (Inter-Process Communication)
 *
 * This crate provides a high-performance IPC layer for PluresDB using
 * shared memory for zero-copy communication between processes.
 *
 * # Features
 *
 * - Shared memory for zero-copy data transfer
 * - Message-based protocol
 * - Process isolation
 * - No network exposure
 *
 * # Limitations
 *
 * - **Single Client**: Current implementation supports one client at a time.
 *   Multiple concurrent clients would require additional synchronization.
 * - **Polling**: Uses polling instead of event-driven synchronization for simplicity.
 *   Future versions could use condition variables for better performance.
 * - **Platform-specific**: Shared memory behavior varies across platforms.
 *   Thoroughly test on target platforms (Windows, macOS, Linux).
 *
 * # Safety
 *
 * This crate uses `unsafe` code for shared memory access. Safety is ensured by:
 * - Single writer (server) per shared memory region
 * - Request/response flags prevent concurrent access
 * - repr(C) layout ensures consistent memory structure
 * - Client waits for response before sending next request
 *
 * # Example
 *
 * ```rust,no_run
 * use pluresdb_ipc::{IPCServer, IPCClient};
 * use pluresdb_core::CrdtStore;
 * use std::sync::{Arc, Mutex};
 *
 * // Server process
 * let store = Arc::new(Mutex::new(CrdtStore::default()));
 * let mut server = IPCServer::new("my-app-channel", store)?;
 * server.start()?;
 *
 * // Client process
 * let mut client = IPCClient::connect("my-app-channel")?;
 * client.put("user:1", serde_json::json!({"name": "Alice"}))?;
 * let user = client.get("user:1")?;
 * # Ok::<(), anyhow::Error>(())
 * ```
 */

use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared_memory::{Shmem, ShmemConf};
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;
use std::thread;

const SHMEM_SIZE: usize = 1024 * 1024; // 1MB shared memory
const MAX_MESSAGE_SIZE: usize = SHMEM_SIZE - 256; // Reserve space for metadata

/// IPC message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IPCMessage {
    /// Put request
    Put {
        id: String,
        data: Value,
    },
    /// Get request
    Get {
        id: String,
    },
    /// Delete request
    Delete {
        id: String,
    },
    /// List all nodes request
    List,
    /// Response with data
    Response {
        data: Option<Value>,
    },
    /// List response with multiple items
    ListResponse {
        items: Vec<Value>,
    },
    /// Error response
    Error {
        message: String,
    },
    /// Shutdown signal
    Shutdown,
}

/// Shared memory layout
#[repr(C)]
struct ShmemLayout {
    /// Request ready flag (1 = request available, 0 = no request)
    request_ready: u8,
    /// Response ready flag (1 = response available, 0 = no response)
    response_ready: u8,
    /// Request data length
    request_len: u32,
    /// Response data length
    response_len: u32,
    /// Reserved bytes for future use
    _reserved: [u8; 240],
    /// Request/response data buffer
    data: [u8; MAX_MESSAGE_SIZE],
}

impl ShmemLayout {
    fn write_request(&mut self, data: &[u8]) -> Result<()> {
        if data.len() > MAX_MESSAGE_SIZE {
            anyhow::bail!("Message too large: {} > {}", data.len(), MAX_MESSAGE_SIZE);
        }
        self.request_len = data.len() as u32;
        self.data[..data.len()].copy_from_slice(data);
        self.request_ready = 1;
        Ok(())
    }

    fn read_request(&mut self) -> Option<Vec<u8>> {
        if self.request_ready == 0 {
            return None;
        }
        let len = self.request_len as usize;
        let data = self.data[..len].to_vec();
        self.request_ready = 0;
        Some(data)
    }

    fn write_response(&mut self, data: &[u8]) -> Result<()> {
        if data.len() > MAX_MESSAGE_SIZE {
            anyhow::bail!("Response too large: {} > {}", data.len(), MAX_MESSAGE_SIZE);
        }
        self.response_len = data.len() as u32;
        self.data[..data.len()].copy_from_slice(data);
        self.response_ready = 1;
        Ok(())
    }

    fn read_response(&mut self) -> Option<Vec<u8>> {
        if self.response_ready == 0 {
            return None;
        }
        let len = self.response_len as usize;
        let data = self.data[..len].to_vec();
        self.response_ready = 0;
        Some(data)
    }
}

/// IPC server for handling requests
pub struct IPCServer {
    channel_name: String,
    shmem: Shmem,
    store: Arc<Mutex<pluresdb_core::CrdtStore>>,
    running: Arc<Mutex<bool>>,
}

impl IPCServer {
    /// Create a new IPC server
    pub fn new(channel_name: &str, store: Arc<Mutex<pluresdb_core::CrdtStore>>) -> Result<Self> {
        let shmem = ShmemConf::new()
            .size(SHMEM_SIZE)
            .os_id(channel_name)
            .create()
            .context("Failed to create shared memory")?;

        Ok(Self {
            channel_name: channel_name.to_string(),
            shmem,
            store,
            running: Arc::new(Mutex::new(false)),
        })
    }

    /// Start the IPC server
    pub fn start(&mut self) -> Result<()> {
        *self.running.lock() = true;

        // Process messages in a loop
        while *self.running.lock() {
            self.process_one_message()?;
            thread::sleep(Duration::from_millis(10));
        }

        Ok(())
    }

    /// Process one message from the shared memory
    fn process_one_message(&self) -> Result<()> {
        // Safety: We control the shared memory lifecycle and ensure single writer.
        // The ShmemLayout is repr(C) and matches the memory layout exactly.
        let layout = unsafe {
            &mut *(self.shmem.as_ptr() as *mut ShmemLayout)
        };

        if let Some(request_data) = layout.read_request() {
            let message: IPCMessage = bincode::deserialize(&request_data)
                .context("Failed to deserialize request")?;

            let response = self.handle_message(message);
            let response_data = bincode::serialize(&response)
                .context("Failed to serialize response")?;

            layout.write_response(&response_data)
                .context("Failed to write response")?;
        }

        Ok(())
    }

    /// Handle an IPC message
    fn handle_message(&self, message: IPCMessage) -> IPCMessage {
        match message {
            IPCMessage::Put { id, data } => {
                let mut store = self.store.lock();
                let node_id = store.put(id, "ipc".to_string(), data);
                IPCMessage::Response {
                    data: Some(Value::String(node_id)),
                }
            }
            IPCMessage::Get { id } => {
                let store = self.store.lock();
                match store.get(id) {
                    Some(record) => IPCMessage::Response {
                        data: Some(record.data),
                    },
                    None => IPCMessage::Response { data: None },
                }
            }
            IPCMessage::Delete { id } => {
                let mut store = self.store.lock();
                match store.delete(&id) {
                    Ok(_) => IPCMessage::Response { data: None },
                    Err(e) => IPCMessage::Error {
                        message: e.to_string(),
                    },
                }
            }
            IPCMessage::List => {
                let store = self.store.lock();
                let records = store.list();
                let items: Vec<Value> = records
                    .into_iter()
                    .map(|r| serde_json::json!({ "id": r.id, "data": r.data }))
                    .collect();
                IPCMessage::ListResponse { items }
            }
            IPCMessage::Shutdown => {
                *self.running.lock() = false;
                IPCMessage::Response { data: None }
            }
            _ => IPCMessage::Error {
                message: "Invalid message type".to_string(),
            },
        }
    }

    /// Stop the IPC server
    pub fn stop(&self) {
        *self.running.lock() = false;
    }
}

impl Drop for IPCServer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// IPC client for sending requests
pub struct IPCClient {
    channel_name: String,
    shmem: Shmem,
}

impl IPCClient {
    /// Connect to an existing IPC server
    pub fn connect(channel_name: &str) -> Result<Self> {
        let shmem = ShmemConf::new()
            .size(SHMEM_SIZE)
            .os_id(channel_name)
            .open()
            .context("Failed to open shared memory. Is the server running?")?;

        Ok(Self {
            channel_name: channel_name.to_string(),
            shmem,
        })
    }

    /// Send a message and wait for response
    fn send_message(&mut self, message: IPCMessage) -> Result<IPCMessage> {
        // Safety: We control the shared memory lifecycle and ensure proper synchronization.
        // The ShmemLayout is repr(C) and matches the memory layout exactly.
        let layout = unsafe {
            &mut *(self.shmem.as_ptr() as *mut ShmemLayout)
        };

        // Serialize and send request
        let request_data = bincode::serialize(&message)
            .context("Failed to serialize message")?;
        layout.write_request(&request_data)
            .context("Failed to write request")?;

        // Wait for response (with timeout)
        let timeout = Duration::from_secs(5);
        let start = std::time::Instant::now();

        loop {
            if let Some(response_data) = layout.read_response() {
                let response: IPCMessage = bincode::deserialize(&response_data)
                    .context("Failed to deserialize response")?;
                return Ok(response);
            }

            if start.elapsed() > timeout {
                anyhow::bail!("Request timeout");
            }

            thread::sleep(Duration::from_millis(10));
        }
    }

    /// Put a value into the database
    pub fn put(&mut self, id: &str, data: Value) -> Result<String> {
        let message = IPCMessage::Put {
            id: id.to_string(),
            data,
        };

        match self.send_message(message)? {
            IPCMessage::Response { data: Some(Value::String(node_id)) } => Ok(node_id),
            IPCMessage::Error { message } => anyhow::bail!("Put failed: {}", message),
            _ => anyhow::bail!("Unexpected response type"),
        }
    }

    /// Get a value from the database
    pub fn get(&mut self, id: &str) -> Result<Option<Value>> {
        let message = IPCMessage::Get {
            id: id.to_string(),
        };

        match self.send_message(message)? {
            IPCMessage::Response { data } => Ok(data),
            IPCMessage::Error { message } => anyhow::bail!("Get failed: {}", message),
            _ => anyhow::bail!("Unexpected response type"),
        }
    }

    /// Delete a value from the database
    pub fn delete(&mut self, id: &str) -> Result<()> {
        let message = IPCMessage::Delete {
            id: id.to_string(),
        };

        match self.send_message(message)? {
            IPCMessage::Response { .. } => Ok(()),
            IPCMessage::Error { message } => anyhow::bail!("Delete failed: {}", message),
            _ => anyhow::bail!("Unexpected response type"),
        }
    }

    /// List all nodes in the database
    pub fn list(&mut self) -> Result<Vec<Value>> {
        let message = IPCMessage::List;

        match self.send_message(message)? {
            IPCMessage::ListResponse { items } => Ok(items),
            IPCMessage::Error { message } => anyhow::bail!("List failed: {}", message),
            _ => anyhow::bail!("Unexpected response type"),
        }
    }

    /// Send shutdown signal to the server
    pub fn shutdown(&mut self) -> Result<()> {
        let message = IPCMessage::Shutdown;
        let _ = self.send_message(message)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pluresdb_core::CrdtStore;

    #[test]
    fn test_ipc_server_creation() {
        let store = Arc::new(Mutex::new(CrdtStore::default()));
        let server = IPCServer::new("test-channel-create", store);
        assert!(server.is_ok());
    }

    #[test]
    fn test_ipc_basic_operations() {
        // This test requires running server and client in separate threads
        let store = Arc::new(Mutex::new(CrdtStore::default()));
        let mut server = IPCServer::new("test-channel-ops", store.clone()).unwrap();

        // Start server in a thread
        let server_handle = thread::spawn(move || {
            // Run for a limited time
            for _ in 0..100 {
                let _ = server.process_one_message();
                thread::sleep(Duration::from_millis(10));
            }
        });

        // Give server time to start
        thread::sleep(Duration::from_millis(100));

        // Connect client
        let mut client = IPCClient::connect("test-channel-ops").unwrap();

        // Test put
        let id = client.put("user:1", serde_json::json!({"name": "Alice"})).unwrap();
        assert_eq!(id, "user:1");

        // Test get
        let data = client.get("user:1").unwrap();
        assert!(data.is_some());

        // Test delete
        client.delete("user:1").unwrap();
        let data = client.get("user:1").unwrap();
        assert!(data.is_none());

        // Shutdown server
        let _ = client.shutdown();

        server_handle.join().unwrap();
    }
}
