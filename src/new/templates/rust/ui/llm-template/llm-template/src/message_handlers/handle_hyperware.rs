use hyperware_process_lib::{
    Address, Response,
    http::server::{HttpServer, WsMessageType, send_ws_push},
    logging::info,
    LazyLoadBlob,
};
use serde_json;
use crate::types::AppState;
use crate::log_message;
use super::make_terminal_address;
use crate::hyperware::process::llm_template::{
    MessageChannel, MessageType, ApiResponse, StateOverview, SuccessResponse
};
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
    // Convert message counts to WIT format
    let counts_by_channel: Vec<(String, u64)> = state.message_counts
        .iter()
        .map(|(k, v)| (format!("{:?}", k), *v as u64))
        .collect();
        
    let response = ApiResponse::Status(StateOverview {
        connected_clients: state.connected_clients.len() as u64,
        message_count: state.message_history.len() as u64,
        message_counts_by_channel: counts_by_channel,
    });
    
    if let Ok(status_json) = serde_json::to_string(&response) {
        for (client_id, _) in &state.connected_clients {
            // Sending WS push to all connected clients
            send_ws_push(
                *client_id,
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
    
    // Return success response
    let response = ApiResponse::Message(SuccessResponse {
        message: "Terminal message logged successfully".to_string(),
    });
    
    Response::new()
        .body(serde_json::to_vec(&response)?)
        .send()?;
    
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
    
    // Return success response
    let response = ApiResponse::Message(SuccessResponse {
        message: "Message logged successfully".to_string(),
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
    // Try to parse the incoming message as a hyper-api-request
    let hyper_request: Result<HyperApiRequest, _> = serde_json::from_slice(body);
    
    let response = match hyper_request {
        Ok(request) => {
            match request {
                HyperApiRequest::GetStatus => {
                    HyperApiResponse::Status(StateOverview {
                        connected_clients: state.connected_clients,
                        message_count: state.message_count,
                        message_counts_by_channel: state.message_counts_by_channel.clone(),
                    })
                },
                HyperApiRequest::GetHistory => {
                    HyperApiResponse::History(state.message_history.clone())
                },
                HyperApiRequest::ClearHistory => {
                    state.message_history.clear();
                    HyperApiResponse::ClearHistory(SuccessResponse {
                        message: "History cleared successfully".to_string(),
                    })
                },
                HyperApiRequest::Message(msg) => {
                    log_message(
                        state,
                        format!("External:{}", source),
                        MessageChannel::External,
                        MessageType::Other(msg.message_type.clone()),
                        Some(msg.content.clone()),
                    );
                    HyperApiResponse::Message(SuccessResponse {
                        message: "Message processed successfully".to_string(),
                    })
                }
            }
        },
        Err(_) => {
            // Fall back to the original behavior for non-hyper messages
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
            HyperApiResponse::ApiError(ErrorResponse {
                code: 400,
                message: "Invalid request format".to_string(),
            })
        }
    };

    Response::new()
        .body(serde_json::to_vec(&response)?)
        .send()?;
    
    Ok(())
}