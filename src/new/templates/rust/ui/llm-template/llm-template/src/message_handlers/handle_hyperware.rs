use std::collections::HashMap;
use hyperware_process_lib::{
    Address, Response,
    http::server::{HttpServer, WsMessageType, send_ws_push},
    logging::info,
    LazyLoadBlob,
};
use serde_json;
use shared_types::{MessageChannel, MessageType, AppState};
use crate::log_message;
use super::make_terminal_address;

// Timer handler, usually used for time-based events
pub fn handle_timer_message(
    _body: &[u8],
    state: &mut AppState,
    _server: &mut HttpServer,
) -> anyhow::Result<()> {
    // Log the timer message
    log_message(
        state,
        "Timer".to_string(),
        MessageChannel::Timer,
        MessageType::TimerTick,
        Some("Timer event received".to_string()),
    );
    info!("Received timer message");
    // Example: Send status updates to all connected websocket clients
    let counts: HashMap<String, usize> = state.message_counts
        .iter()
        .map(|(k, v)| (format!("{:?}", k), *v))
        .collect();
        
    let status_update = serde_json::json!({
        "type": "status_update",
        "connected_clients": state.connected_clients.len(),
        "message_count": state.message_history.len(),
        "message_counts_by_channel": counts
    });
    
    if let Ok(status_json) = serde_json::to_string(&status_update) {
        for channel_id in state.connected_clients.keys() {
            // Sending WS push to all connected clients
            send_ws_push(
                *channel_id,
                WsMessageType::Text,
                LazyLoadBlob {
                    mime: Some("application/json".to_string()),
                    bytes: status_json.as_bytes().to_vec(),
                },
            );
        }
    }
    
    Ok(())
}

// Terminal handler, usually used for debugging purposes, state checking
pub fn handle_terminal_message(
    body: &[u8],
    state: &mut AppState,
    _server: &mut HttpServer,
) -> anyhow::Result<()> {
    // Log the terminal message
    let content = if let Ok(str) = std::str::from_utf8(body) {
        Some(str.to_string())
    } else {
        Some(format!("Binary data: {} bytes", body.len()))
    };
    
    log_message(
        state,
        "Terminal".to_string(),
        MessageChannel::Terminal,
        MessageType::TerminalCommand,
        content,
    );
    
    // Process terminal commands if needed
    
    Ok(())
}

pub fn handle_internal_message(
    source: &Address,
    body: &[u8],
    state: &mut AppState,
    server: &mut HttpServer,
) -> anyhow::Result<()> {
    // Terminal messages are usually used for debugging purposes (checking state, etc.)
    if source == &make_terminal_address(source) {
        return handle_terminal_message(body, state, server);
    }
    
    // Log the internal message
    let content = if let Ok(str) = std::str::from_utf8(body) {
        Some(str.to_string())
    } else {
        Some(format!("Binary data: {} bytes", body.len()))
    };
    
    log_message(
        state,
        format!("Internal:{}", source),
        MessageChannel::Internal,
        MessageType::LocalRequest,
        content,
    );
    
    // Simple response for internal messages
    let response = serde_json::json!({
        "status": "ok",
        "message": "Message logged",
        "message_count": state.message_history.len()
    });

    Response::new()
        .body(serde_json::to_vec(&response)?)
        .send()?;
    
    Ok(())
}

pub fn handle_external_message(
    source: &Address,
    body: &[u8],
    state: &mut AppState,
    _server: &mut HttpServer,
) -> anyhow::Result<()> {
    // Log the external message
    let content = if let Ok(str) = std::str::from_utf8(body) {
        Some(str.to_string())
    } else {
        Some(format!("Binary data: {} bytes", body.len()))
    };
    
    log_message(
        state,
        format!("External:{}", source),
        MessageChannel::External,
        MessageType::ResponseReceived,
        content,
    );
    
    
    // Simple response for external messages
    let response = serde_json::json!({
            "status": "ok",
            "message": "Message logged",
            "message_count": state.message_history.len()
    });
       
    Response::new()
        .body(serde_json::to_vec(&response)?)
        .send()?;
    
    
    Ok(())
}