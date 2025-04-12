use crate::hyperware::process::llm_template::{MessageLog, MessageChannel, MessageType};

/// Represents the application state
#[derive(Debug, Clone, Default)]
pub struct AppState {
    /// Tracks message history for all channels
    pub message_history: Vec<MessageLog>,
    /// Message counts by channel
    pub message_counts: Vec<(MessageChannel, usize)>,
    /// Configuration settings
    pub config: AppConfig,
    /// Connected WebSocket clients (channel_id -> path)
    pub connected_clients: Vec<(u32, String)>,
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

impl AppState {
    /// Increment count for a channel
    pub fn increment_channel_count(&mut self, channel: MessageChannel) {
        if let Some(count) = self.message_counts.iter_mut().find(|(ch, _)| *ch == channel) {
            count.1 += 1;
        } else {
            self.message_counts.push((channel, 1));
        }
    }

    /// Add a client connection
    pub fn add_client(&mut self, channel_id: u32, path: String) {
        self.connected_clients.push((channel_id, path));
    }

    /// Remove a client connection
    pub fn remove_client(&mut self, channel_id: u32) {
        self.connected_clients.retain(|(id, _)| *id != channel_id);
    }

    /// Get client path
    pub fn get_client_path(&self, channel_id: u32) -> Option<&str> {
        self.connected_clients
            .iter()
            .find(|(id, _)| *id == channel_id)
            .map(|(_, path)| path.as_str())
    }

    /// Clear message counts
    pub fn clear_counts(&mut self) {
        self.message_counts.clear();
    }
}