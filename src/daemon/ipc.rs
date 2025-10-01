// Inter-process communication via Unix domain sockets with length-prefixed JSON protocol

use crate::error::{Result, YinxError};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

/// Maximum message size (10MB)
const MAX_MESSAGE_SIZE: u32 = 10 * 1024 * 1024;

/// IPC message types sent from shell hooks or CLI to daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum IpcMessage {
    /// Capture output from a command execution
    Capture {
        session_id: String,
        timestamp: i64,
        command: String,
        output: String,
        exit_code: i32,
        cwd: String,
    },
    /// Request daemon status
    Status,
    /// Request daemon to stop
    Stop,
    /// Query for data
    Query { query: String, limit: usize },
}

/// IPC response message sent from daemon back to client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl IpcResponse {
    /// Create a successful response
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: Some(message.into()),
            data: None,
        }
    }

    /// Create a successful response with data
    pub fn success_with_data(data: serde_json::Value) -> Self {
        Self {
            success: true,
            message: None,
            data: Some(data),
        }
    }

    /// Create an error response
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: Some(message.into()),
            data: None,
        }
    }
}

/// Unix domain socket server for IPC
pub struct IpcServer {
    socket_path: PathBuf,
    listener: Option<UnixListener>,
}

impl IpcServer {
    /// Create a new IPC server
    pub fn new(socket_path: PathBuf) -> Self {
        Self {
            socket_path,
            listener: None,
        }
    }

    /// Bind to the socket path and start listening
    pub async fn bind(&mut self) -> Result<()> {
        // Remove existing socket file if it exists
        if self.socket_path.exists() {
            std::fs::remove_file(&self.socket_path).map_err(|e| YinxError::Io {
                source: e,
                context: format!("Failed to remove existing socket: {:?}", self.socket_path),
            })?;
        }

        // Ensure parent directory exists
        if let Some(parent) = self.socket_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| YinxError::Io {
                source: e,
                context: format!("Failed to create socket directory: {:?}", parent),
            })?;
        }

        // Bind to socket
        let listener = UnixListener::bind(&self.socket_path).map_err(|e| YinxError::Io {
            source: e,
            context: format!("Failed to bind to socket: {:?}", self.socket_path),
        })?;

        self.listener = Some(listener);

        tracing::info!("IPC server listening on {:?}", self.socket_path);
        Ok(())
    }

    /// Accept incoming connections
    pub async fn accept(&mut self) -> Result<UnixStream> {
        let listener = self
            .listener
            .as_ref()
            .ok_or_else(|| YinxError::Daemon("Server not bound".to_string()))?;

        let (stream, _addr) = listener.accept().await.map_err(|e| YinxError::Io {
            source: e,
            context: "Failed to accept connection".to_string(),
        })?;

        Ok(stream)
    }

    /// Shutdown the server and clean up socket file
    pub fn shutdown(&self) -> Result<()> {
        if self.socket_path.exists() {
            std::fs::remove_file(&self.socket_path).map_err(|e| YinxError::Io {
                source: e,
                context: format!("Failed to remove socket: {:?}", self.socket_path),
            })?;
        }
        Ok(())
    }

    /// Get the socket path
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }
}

/// Read a length-prefixed message from a Unix stream
pub async fn read_message(stream: &mut UnixStream) -> Result<IpcMessage> {
    // Read 4-byte length prefix
    let length = stream.read_u32().await.map_err(|e| YinxError::Io {
        source: e,
        context: "Failed to read message length".to_string(),
    })?;

    // Validate length
    if length > MAX_MESSAGE_SIZE {
        return Err(YinxError::Daemon(format!(
            "Message too large: {} bytes (max: {})",
            length, MAX_MESSAGE_SIZE
        )));
    }

    // Read message payload
    let mut buffer = vec![0u8; length as usize];
    stream
        .read_exact(&mut buffer)
        .await
        .map_err(|e| YinxError::Io {
            source: e,
            context: "Failed to read message payload".to_string(),
        })?;

    // Deserialize JSON
    let message: IpcMessage = serde_json::from_slice(&buffer).map_err(|e| YinxError::Json {
        source: e,
        context: "Failed to deserialize IPC message".to_string(),
    })?;

    Ok(message)
}

/// Write a length-prefixed message to a Unix stream
pub async fn write_response(stream: &mut UnixStream, response: &IpcResponse) -> Result<()> {
    // Serialize to JSON
    let payload = serde_json::to_vec(response).map_err(|e| YinxError::Json {
        source: e,
        context: "Failed to serialize IPC response".to_string(),
    })?;

    // Check size
    if payload.len() > MAX_MESSAGE_SIZE as usize {
        return Err(YinxError::Daemon(format!(
            "Response too large: {} bytes (max: {})",
            payload.len(),
            MAX_MESSAGE_SIZE
        )));
    }

    // Write length prefix (4 bytes, big-endian)
    let length = payload.len() as u32;
    stream.write_u32(length).await.map_err(|e| YinxError::Io {
        source: e,
        context: "Failed to write response length".to_string(),
    })?;

    // Write payload
    stream
        .write_all(&payload)
        .await
        .map_err(|e| YinxError::Io {
            source: e,
            context: "Failed to write response payload".to_string(),
        })?;

    // Flush
    stream.flush().await.map_err(|e| YinxError::Io {
        source: e,
        context: "Failed to flush response".to_string(),
    })?;

    Ok(())
}

/// IPC client for sending messages to the daemon
pub struct IpcClient {
    socket_path: PathBuf,
}

impl IpcClient {
    /// Create a new IPC client
    pub fn new(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }

    /// Connect to the daemon and send a message, returning the response
    pub async fn send(&self, message: &IpcMessage) -> Result<IpcResponse> {
        // Connect to socket
        let mut stream =
            UnixStream::connect(&self.socket_path)
                .await
                .map_err(|e| YinxError::Io {
                    source: e,
                    context: format!("Failed to connect to daemon at {:?}", self.socket_path),
                })?;

        // Serialize message
        let payload = serde_json::to_vec(message).map_err(|e| YinxError::Json {
            source: e,
            context: "Failed to serialize IPC message".to_string(),
        })?;

        // Write length prefix
        let length = payload.len() as u32;
        stream.write_u32(length).await.map_err(|e| YinxError::Io {
            source: e,
            context: "Failed to write message length".to_string(),
        })?;

        // Write payload
        stream
            .write_all(&payload)
            .await
            .map_err(|e| YinxError::Io {
                source: e,
                context: "Failed to write message payload".to_string(),
            })?;

        stream.flush().await.map_err(|e| YinxError::Io {
            source: e,
            context: "Failed to flush message".to_string(),
        })?;

        // Read response length
        let response_length = stream.read_u32().await.map_err(|e| YinxError::Io {
            source: e,
            context: "Failed to read response length".to_string(),
        })?;

        if response_length > MAX_MESSAGE_SIZE {
            return Err(YinxError::Daemon(format!(
                "Response too large: {} bytes",
                response_length
            )));
        }

        // Read response payload
        let mut response_buffer = vec![0u8; response_length as usize];
        stream
            .read_exact(&mut response_buffer)
            .await
            .map_err(|e| YinxError::Io {
                source: e,
                context: "Failed to read response payload".to_string(),
            })?;

        // Deserialize response
        let response: IpcResponse =
            serde_json::from_slice(&response_buffer).map_err(|e| YinxError::Json {
                source: e,
                context: "Failed to deserialize IPC response".to_string(),
            })?;

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_message_serialization() {
        let msg = IpcMessage::Capture {
            session_id: "test-session".to_string(),
            timestamp: 1234567890,
            command: "ls -la".to_string(),
            output: "total 0\ndrwxr-xr-x".to_string(),
            exit_code: 0,
            cwd: "/home/user".to_string(),
        };

        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: IpcMessage = serde_json::from_str(&json).unwrap();

        match deserialized {
            IpcMessage::Capture { command, .. } => assert_eq!(command, "ls -la"),
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_ipc_response_creation() {
        let success = IpcResponse::success("Operation completed");
        assert!(success.success);
        assert_eq!(success.message.unwrap(), "Operation completed");

        let error = IpcResponse::error("Operation failed");
        assert!(!error.success);
        assert_eq!(error.message.unwrap(), "Operation failed");
    }
}
