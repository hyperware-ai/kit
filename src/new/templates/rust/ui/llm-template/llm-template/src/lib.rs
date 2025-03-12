use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use hyperware_process_lib::logging::{error, info, init_logging, Level};
use hyperware_process_lib::{
    await_message, call_init,
    http::server::{
        HttpBindingConfig, HttpServer,
        WsBindingConfig,
    },
    Address, Message
};

mod types;
use types::*;

mod message_handlers;
use message_handlers::*;

wit_bindgen::generate!({
    path: "target/wit",
    world: "llm-template-template-dot-os-v0",
    generate_unused_types: true,
    additional_derives: [serde::Deserialize, serde::Serialize, process_macros::SerdeJsonInto],
});

const WS_PATH: &str = "/";
fn bind_http_endpoints(server: &mut HttpServer) {
    let public_config = HttpBindingConfig::new(false, false, false, None);
    let authenticated_config = HttpBindingConfig::new(true, false, false, None);
    
    // Define API paths
    let public_paths = vec![
        "/api",              // Base API path
        "/api/status",       // GET status endpoint
        "/api/history",      // GET history endpoint
    ];
    
    // Bind public paths
    for path in public_paths {
        server.bind_http_path(path, public_config.clone())
            .expect(&format!("failed to bind HTTP API path: {}", path));
    }
    
    // Bind authenticated paths (if needed)
    let authenticated_paths = vec![
        "/api/clear-history",    // For authenticated clear history requests
    ];
    
    for path in authenticated_paths {
        server.bind_http_path(path, authenticated_config.clone())
            .expect(&format!("failed to bind authenticated HTTP API path: {}", path));
    }
}

fn get_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// Helper function to log a message and update counts
fn log_message(
    state: &mut AppState,
    source: String,
    channel: MessageChannel,
    message_type: MessageType,
    content: Option<String>,
) {
    // Add to message history
    state.message_history.push(MessageLog {
        source,
        channel: channel.clone(),
        message_type,
        content: if state.config.log_content { content } else { None },
        timestamp: get_timestamp(),
    });
    
    // Update message count for this channel
    *state.message_counts.entry(channel).or_insert(0) += 1;
    
    // Trim history if needed
    if state.message_history.len() > state.config.max_history {
        state.message_history.remove(0);
    }
}


fn handle_message(
    message: &Message,
    our: &Address,
    state: &mut AppState,
    server: &mut HttpServer,
) -> anyhow::Result<()> {
    match message.source() {
        // Handling HTTP and WS requests
        source if source == &make_http_address(our) => {
            println!("got http request");
            handle_http_server_request(our, message.body(), state, server)
        }
        // Handling timer messages (for time-based events)
        source if source == &make_timer_address(our) => {
            handle_timer_message(message.body(), state, server)
        }
        // Handling internal messages (messages from other processes on the same node)
        source if source.node == our.node => {
            handle_internal_message(source, message.body(), state, server)
        }
        // Handling terminal messages (for debugging purposes)
        source if source == &make_terminal_address(our) => {
            handle_terminal_message(message.body(), state, server)
        }
        // Handling external messages (messages from other applications/hyperdrives)
        source => handle_external_message(
            source, 
            message.body(), 
            state, 
            server
        ),
    }
}

call_init!(init);
fn init(our: Address) {
    // Arguments: file level, terminal level, remote, terminal_level_mapping, max_log_file_size
    init_logging(Level::DEBUG, Level::INFO, None, None, None).unwrap();
    info!("begin");

    // Initialize application state
    let mut state = AppState {
        config: AppConfig {
            max_history: 100,
            log_content: true,
        },
        message_counts: HashMap::new(),
        ..Default::default()
    };

    let mut server = HttpServer::new(5);
    let http_config = HttpBindingConfig::default();
    bind_http_endpoints(&mut server);

    // Bind UI files to routes with index.html at "/"; WS to "/"
    server
        .serve_ui("ui", vec!["/"], http_config.clone())
        .expect("failed to serve UI");


    server
        .bind_ws_path(WS_PATH, WsBindingConfig::default())
        .expect("failed to bind WS API");

    // Log initialization
    log_message(
        &mut state,
        "System".to_string(),
        MessageChannel::Internal,
        MessageType::Other("Initialization".to_string()),
        Some("Application started".to_string()),
    );

    loop {
        match await_message() {
            Err(send_error) => {
                error!("got SendError: {send_error}");
                log_message(
                    &mut state,
                    "System".to_string(),
                    MessageChannel::Internal,
                    MessageType::Other("Error".to_string()),
                    Some(format!("SendError: {}", send_error)),
                );
            },
            Ok(ref message) => {
                match handle_message(message, &our, &mut state, &mut server) {
                    Ok(_) => {}
                    Err(e) => {
                        error!("got error while handling message: {e:?}");
                        log_message(
                            &mut state,
                            "System".to_string(),
                            MessageChannel::Internal,
                            MessageType::Other("Error".to_string()),
                            Some(format!("Error handling message: {}", e)),
                        );
                    }
                }
            }
        }
    }
}
