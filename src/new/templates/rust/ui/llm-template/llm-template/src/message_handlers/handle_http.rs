use hyperware_process_lib::{
    get_blob, Address, 
    http::server::{
        send_response, HttpServer, HttpServerRequest, StatusCode, send_ws_push, WsMessageType
    },
    http::Method,
    logging::info,
    LazyLoadBlob, last_blob,
};
use anyhow::anyhow;
use crate::hyperware::process::llm_template::{
    ApiRequest, ApiResponse, StateOverview, SuccessResponse, ErrorResponse, MessageChannel, MessageType
};
use crate::types::AppState;
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
            state.add_client(channel_id, path.clone());
            
            // Log the connection
            log_message(
                state,
                format!("WebSocket:{}", channel_id),
                MessageChannel::Websocket,
                MessageType::WebsocketOpen,
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
                MessageChannel::Websocket,
                MessageType::WebsocketClose,
                Some(format!("Channel ID: {}", channel_id)),
            );
            
            // Remove the closed connection
            state.remove_client(channel_id);
            server.handle_websocket_close(channel_id);
            Ok(())
        },
        HttpServerRequest::WebSocketPush {channel_id, .. } => {
            handle_websocket_push(state, channel_id)    
        },
        HttpServerRequest::Http(http_request) => {
            // Get the HTTP method and path
            let Ok(method) = http_request.method() else {
                return Err(anyhow!("HTTP request with no method"));
            };

            let path = http_request.path().unwrap_or_default();
            println!("HTTP Request: {} {}", method, path);
            info!("HTTP Request: {} {}", method, path);
            
            // Handle different HTTP methods
            match method {
                Method::GET => {
                    // Handle GET requests based on path
                    match path.as_str() {
                        "/api/status" => {
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
                            
                            log_message(
                                state,
                                "HTTP:GET".to_string(),
                                MessageChannel::HttpApi,
                                MessageType::HttpGet,
                                Some("Status request".to_string()),
                            );
                            
                            send_response(StatusCode::OK, None, serde_json::to_vec(&response)?);
                        },
                        "/api/history" => {
                            // Use MessageLog type directly since it's WIT-compatible
                            let data = state.message_history.clone();
                            let response = ApiResponse::History(data);
                            
                            log_message(
                                state,
                                "HTTP:GET".to_string(),
                                MessageChannel::HttpApi,
                                MessageType::HttpGet,
                                Some("History request".to_string()),
                            );
                            
                            send_response(StatusCode::OK, None, serde_json::to_vec(&response)?);
                        },
                        _ => {
                            println!("Non-API path: {}", path);
                        }
                    }
                },
                Method::POST => {
                    // For POST requests, we need to parse the body
                    let Some(blob) = last_blob() else {
                        let error_response = ApiResponse::ApiError(ErrorResponse {
                            code: 400,
                            message: "No request body".to_string(),
                        });
                        send_response(StatusCode::BAD_REQUEST, None, serde_json::to_vec(&error_response)?);
                        return Ok(());
                    };
                    
                    let Ok(request_str) = std::str::from_utf8(&blob.bytes()) else {
                        let error_response = ApiResponse::ApiError(ErrorResponse {
                            code: 400,
                            message: "Invalid UTF-8 in request body".to_string(),
                        });
                        send_response(StatusCode::BAD_REQUEST, None, serde_json::to_vec(&error_response)?);
                        return Ok(());
                    };
                    
                    info!("Request body: {}", request_str);
                    
                    // Try to parse the API request
                    match serde_json::from_str::<ApiRequest>(request_str) {
                        Ok(api_request) => {
                            match api_request {
                                ApiRequest::ClearHistory => {
                                    // Clear the history
                                    state.message_history.clear();
                                    state.clear_counts();
                                    
                                    log_message(
                                        state,
                                        "HTTP:POST".to_string(),
                                        MessageChannel::HttpApi,
                                        MessageType::HttpPost,
                                        Some("History cleared".to_string()),
                                    );
                                    
                                    let response = ApiResponse::ClearHistory(SuccessResponse {
                                        message: "History cleared successfully".to_string(),
                                    });
                                    
                                    send_response(StatusCode::OK, None, serde_json::to_vec(&response)?);
                                },
                                ApiRequest::Message(msg) => {
                                    // Log a custom message
                                    log_message(
                                        state,
                                        "HTTP:Custom".to_string(),
                                        MessageChannel::HttpApi,
                                        MessageType::Other(msg.message_type.clone()),
                                        Some(msg.content.clone()),
                                    );
                                    
                                    let response = ApiResponse::Message(SuccessResponse {
                                        message: "Custom message logged successfully".to_string(),
                                    });
                                    
                                    send_response(StatusCode::OK, None, serde_json::to_vec(&response)?);
                                },
                                _ => {
                                    // Invalid request - should use GET endpoints instead
                                    let error_response = ApiResponse::ApiError(ErrorResponse {
                                        code: 400,
                                        message: "Invalid request type. Use GET endpoints for status and history.".to_string(),
                                    });
                                    send_response(StatusCode::BAD_REQUEST, None, serde_json::to_vec(&error_response)?);
                                }
                            }
                        },
                        Err(err) => {
                            let error_response = ApiResponse::ApiError(ErrorResponse {
                                code: 400,
                                message: format!("Invalid request format: {}", err),
                            });
                            send_response(StatusCode::BAD_REQUEST, None, serde_json::to_vec(&error_response)?);
                        }
                    }
                },
                _ => {
                    let error_response = ApiResponse::ApiError(ErrorResponse {
                        code: 405,
                        message: "Method not allowed".to_string(),
                    });
                    send_response(StatusCode::METHOD_NOT_ALLOWED, None, serde_json::to_vec(&error_response)?);
                }
            }
            Ok(())
        }
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
    
    // Process the websocket message
    if let Ok(message_str) = std::str::from_utf8(&blob.bytes()) {
        if let Ok(api_request) = serde_json::from_str::<ApiRequest>(message_str) {
            match api_request {
                ApiRequest::ClearHistory => {
                    // Clear the history
                    state.message_history.clear();
                    state.clear_counts();
                    
                    // Log the action
                    log_message(
                        state,
                        "WebSocket:Clear".to_string(),
                        MessageChannel::Websocket,
                        MessageType::WebsocketPushA,
                        Some("History cleared".to_string()),
                    );
                    
                    // Create a success response
                    let response = ApiResponse::ClearHistory(SuccessResponse {
                        message: "History cleared successfully".to_string(),
                    });
                    
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
                },
                _ => {
                    // Handle all other request types
                    let response = match api_request {
                        ApiRequest::GetStatus => {
                            // Convert message counts to WIT format
                            let counts_by_channel: Vec<(String, u64)> = state.message_counts
                                .iter()
                                .map(|(k, v)| (format!("{:?}", k), *v as u64))
                                .collect();

                            ApiResponse::Status(StateOverview {
                                connected_clients: state.connected_clients.len() as u64,
                                message_count: state.message_history.len() as u64,
                                message_counts_by_channel: counts_by_channel,
                            })
                        },
                        ApiRequest::GetHistory => {
                            ApiResponse::History(state.message_history.clone())
                        },
                        ApiRequest::Message(msg) => {
                            // Log a custom message type
                            log_message(
                                state,
                                "WebSocket:Custom".to_string(),
                                MessageChannel::Websocket,
                                MessageType::WebsocketPushB,
                                Some(format!("Type: {}, Content: {}", msg.message_type, msg.content)),
                            );
                            
                            ApiResponse::Message(SuccessResponse {
                                message: "Custom message logged successfully".to_string(),
                            })
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