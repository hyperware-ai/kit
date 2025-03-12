use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Message source/channel type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MessageChannel {
    /// WebSocket messages
    WebSocket,
    /// HTTP API requests
    HttpApi,
    /// Internal process messages
    Internal,
    /// External node messages
    External,
    /// Timer events
    Timer,
    /// Terminal commands
    Terminal,
}

/// Message type for categorization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    /// WebSocket connection opened
    WebSocketOpen,
    /// WebSocket connection closed
    WebSocketClose,
    /// WebSocket message received
    WebSocketPushA,
    /// Another type of WebSocket message
    WebSocketPushB,
    /// HTTP GET request
    HttpGet,
    /// HTTP POST request
    HttpPost,
    /// Timer tick event
    TimerTick,
    /// Local process request
    LocalRequest,
    /// Remote node request
    RemoteRequest,
    /// Response to our request
    ResponseReceived,
    /// Terminal command
    TerminalCommand,
    /// Other message type
    Other(String),
}

/// Log entry for tracking messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageLog {
    /// Source of the message
    pub source: String,
    /// Channel the message came through
    pub channel: MessageChannel,
    /// Type of the message
    pub message_type: MessageType,
    /// Message content (if available)
    pub content: Option<String>,
    /// Timestamp when the message was received
    pub timestamp: u64,
}

/// Represents the application state
#[derive(Debug, Clone, Default)]
pub struct AppState {
    /// Tracks message history for all channels
    pub message_history: Vec<MessageLog>,
    /// Message counts by channel
    pub message_counts: HashMap<MessageChannel, usize>,
    /// Configuration settings
    pub config: AppConfig,
    /// Connected WebSocket clients (channel_id -> path)
    pub connected_clients: HashMap<u32, String>,
}

/// Configuration for the application
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Maximum number of messages to keep in history
    pub max_history: usize,
    /// Whether to log message content
    pub log_content: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            max_history: 100,
            log_content: true,
        }
    }
}

/// HTTP API request types with a more RESTful approach
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiRequest {
    /// Get system status
    GetStatus,
    /// Get message history
    GetHistory,
    /// Clear history
    ClearHistory,
    /// Custom message for testing
    CustomMessage { message_type: String, content: String },
}

/// HTTP API response types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiResponse {
    /// Response with message history
    History { 
        messages: Vec<MessageLog> 
    },
    /// Response with message counts
    MessageCounts { 
        counts: HashMap<String, usize> 
    },
    /// Status response
    Status { 
        connected_clients: usize,
        message_count: usize,
        message_counts_by_channel: HashMap<String, usize>
    },
    /// Success response
    Success { 
        message: String 
    },
    /// Error response
    Error { 
        code: u16, 
        message: String 
    },
}
