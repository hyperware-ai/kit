use std::collections::HashMap;
use hyperware_process_lib::{
    get_blob, Address,
    http::server::{
        send_response, HttpServer, HttpServerRequest, StatusCode, send_ws_push, WsMessageType, IncomingHttpRequest
    },
    logging::{info, warn},
    LazyLoadBlob,
};
use serde_json::{self, Value};

use crate::types::*;
use crate::log_message;

pub fn handle_http_server_request(
    _our: &Address,
    body: &[u8],
    state: &mut AppState,
    server: &mut HttpServer,
) -> anyhow::Result<()> {
    let Ok(request) = serde_json::from_slice::<HttpServerRequest>(body) else {
        // Fail quietly if we can't parse the request
        info!("couldn't parse message from http_server: {body:?}");
        return Ok(());
    };

    match request {
        HttpServerRequest::WebSocketOpen {
            ref path,
            channel_id,
        } => {            
            // Track the new websocket connection
            state.connected_clients.insert(channel_id, path.clone());
            
            // Log the connection
            log_message(
                state,
                format!("WebSocket:{}", channel_id),
                MessageChannel::WebSocket,
                MessageType::WebSocketOpen,
                Some(format!("Path: {}", path)),
            );
            
            server.handle_websocket_open(path, channel_id);
            Ok(())
        },
        HttpServerRequest::WebSocketClose(channel_id) => {
            // Log the disconnection
            log_message(
                state,
                format!("WebSocket:{}", channel_id),
                MessageChannel::WebSocket,
                MessageType::WebSocketClose,
                Some(format!("Channel ID: {}", channel_id)),
            );
            
            // Remove the closed connection
            state.connected_clients.remove(&channel_id);
            server.handle_websocket_close(channel_id);
            Ok(())
        },
        HttpServerRequest::WebSocketPush {channel_id, .. } => {
            handle_websocket_push(state, channel_id)    
        },
        HttpServerRequest::Http(request)=> {
            println!("got http request");
            println!("request: {:?}", request.clone());
            let method = request.method().unwrap().as_str().to_string();
            let path = request.path().unwrap_or_default();
            let request = get_http_request(request)?;
            let result = handle_request_inner(request, state);
            
            match result {
                Ok(response) => {
                    let headers = HashMap::from([(
                        "Content-Type".to_string(),
                        "application/json".to_string(),
                    )]);
                    
                    send_response(
                        StatusCode::OK,
                        Some(headers),
                        serde_json::to_vec(&response)?,
                    );
                    Ok(())
                },
                Err(err) => {
                    let headers = HashMap::from([(
                        "Content-Type".to_string(),
                        "application/json".to_string(),
                    )]);
                    
                    let error_response = ApiResponse::Error {
                        code: 500,
                        message: err.to_string(),
                    };
                    
                    send_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Some(headers),
                        serde_json::to_vec(&error_response)?,
                    );
                    Ok(())
                }
            }
        }
    }
}
fn get_http_request(incoming_request: IncomingHttpRequest) -> anyhow::Result<ApiRequest> {
    let method = incoming_request.method().unwrap().as_str().to_string();
    let path = incoming_request.path().unwrap_or_default();
    
    // For GET requests, determine the API request type from the path
    if method == "GET" {
        match path.as_str() {
            "/api/status" => return Ok(ApiRequest::GetStatus),
            "/api/history" => return Ok(ApiRequest::GetHistory),
            _ => return Err(anyhow::anyhow!("Unknown GET endpoint: {}", path))
        }
    }
    
    // For non-GET methods, process the request body
    let Some(blob) = get_blob() else {
        return Err(anyhow::anyhow!("No request body"));
    };
    
    let Ok(request_str) = std::str::from_utf8(&blob.bytes()) else {
        return Err(anyhow::anyhow!("Invalid UTF-8 in request body"));
    };
    
    match serde_json::from_str::<ApiRequest>(request_str) {
        Ok(req) => Ok(req),
        Err(err) => Err(anyhow::anyhow!("Invalid request format: {}", err))
    }
}

fn handle_request_inner(request: ApiRequest, state: &mut AppState) -> anyhow::Result<Value> {
    // Process based on the ApiRequest type
    match request {
        ApiRequest::GetStatus => {
            // Return status information
            let counts: HashMap<String, usize> = state.message_counts
                .iter()
                .map(|(k, v)| (format!("{:?}", k), *v))
                .collect();

            let response = ApiResponse::Status { 
                connected_clients: state.connected_clients.len(),
                message_count: state.message_history.len(),
                message_counts_by_channel: counts
            };
            
            log_message(
                state,
                "HTTP:GET".to_string(),
                MessageChannel::HttpApi,
                MessageType::HttpGet,
                Some("Status request".to_string()),
            );

            // Convert directly to a Value
            Ok(serde_json::to_value(&response)?)
        },
        ApiRequest::GetHistory => {
            let response = ApiResponse::History { 
                messages: state.message_history.clone() 
            };
            
            log_message(
                state,
                "HTTP:GET".to_string(),
                MessageChannel::HttpApi,
                MessageType::HttpGet,
                Some("History request".to_string()),
            );
            
            // Convert directly to a Value
            Ok(serde_json::to_value(&response)?)
        },
        ApiRequest::ClearHistory => {
            // Handle clear history request
            state.message_history.clear();
            state.message_counts.clear();
            
            // Log the action
            log_message(
                state,
                "HTTP:POST".to_string(),
                MessageChannel::HttpApi,
                MessageType::HttpPost,
                Some("History cleared".to_string()),
            );
            
            let response = ApiResponse::Success { 
                message: "History cleared successfully".to_string() 
            };
            
            // Convert directly to a Value
            Ok(serde_json::to_value(&response)?)
        },
        ApiRequest::CustomMessage { message_type, content } => {
            // Log a custom message
            log_message(
                state,
                "HTTP:Custom".to_string(),
                MessageChannel::HttpApi,
                MessageType::Other(message_type.clone()),
                Some(content.clone()),
            );
            
            let response = ApiResponse::Success {   
                message: "Custom message logged successfully".to_string() 
            };
            
            // Convert directly to a Value
            Ok(serde_json::to_value(&response)?)
        },
    }
}

fn handle_websocket_push(state: &mut AppState, channel_id: u32) -> anyhow::Result<()> {
    let Some(blob) = get_blob() else {
        return Ok(());
    };

    // Log the message
    let content = if let Ok(str) = std::str::from_utf8(&blob.bytes()) {
        Some(str.to_string())
    } else {
        Some(format!("Binary data: {} bytes", blob.bytes().len()))
    };
    
    log_message(
        state,
        "WebSocket".to_string(),
        MessageChannel::WebSocket,
        MessageType::WebSocketPushA,
        content,
    );
    
    // Process the websocket message
    if let Ok(message_str) = std::str::from_utf8(&blob.bytes()) {
        if let Ok(api_request) = serde_json::from_str::<ApiRequest>(message_str) {
            match api_request {
                ApiRequest::ClearHistory => {
                    // Clear the history
                    state.message_history.clear();
                    state.message_counts.clear();
                    
                    // Log the action
                    log_message(
                        state,
                        "WebSocket:Clear".to_string(),
                        MessageChannel::WebSocket,
                        MessageType::WebSocketPushB,
                        Some("History cleared".to_string()),
                    );
                    
                    // Create a success response
                    let response = ApiResponse::Success { 
                        message: "History cleared successfully".to_string() 
                    };
                    
                    // Convert to JSON
                    if let Ok(response_json) = serde_json::to_string(&response) {
                        // Send to all connected clients
                        for &client_id in state.connected_clients.keys() {
                            send_ws_push(
                                client_id,
                                WsMessageType::Text,
                                LazyLoadBlob {
                                    mime: Some("application/json".to_string()),
                                    bytes: response_json.as_bytes().to_vec(),
                                },
                            );
                        }
                        
                        info!("Sent clear history notification to all clients");
                    }
                },
                _ => {
                    // Handle all other request types
                    let response = match api_request {
                        ApiRequest::GetStatus => {
                            let counts: HashMap<String, usize> = state.message_counts
                                .iter()
                                .map(|(k, v)| (format!("{:?}", k), *v))
                                .collect();

                            ApiResponse::Status { 
                                connected_clients: state.connected_clients.len(),
                                message_count: state.message_history.len(),
                                message_counts_by_channel: counts
                            }
                        },
                        ApiRequest::GetHistory => {
                            ApiResponse::History { 
                                messages: state.message_history.clone() 
                            }
                        },
                        ApiRequest::CustomMessage { message_type, content } => {
                            // Log a custom message type
                            log_message(
                                state,
                                "WebSocket:Custom".to_string(),
                                MessageChannel::WebSocket,
                                MessageType::WebSocketPushB,
                                Some(format!("Type: {}, Content: {}", message_type, content)),
                            );
                            
                            ApiResponse::Success { 
                                message: "Custom message logged successfully".to_string() 
                            }
                        },
                        // We already handled ClearHistory above
                        ApiRequest::ClearHistory => unreachable!(),
                    };
                    
                    // Convert to JSON and send only to the requesting client
                    if let Ok(response_json) = serde_json::to_string(&response) {
                        info!("Sending WS response to client {}: {}", channel_id, response_json);
                        
                        send_ws_push(
                            channel_id,
                            WsMessageType::Text,
                            LazyLoadBlob {
                                mime: Some("application/json".to_string()),
                                bytes: response_json.as_bytes().to_vec(),
                            },
                        );
                    }
                }
            }
        }
    }
    
    Ok(())
}