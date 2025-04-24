use serde::{Serialize, Deserialize};
use hyperware_process_lib::{
    LazyLoadBlob,
    http::server::{WsMessageType, send_ws_push}
};
use hyperprocess_macro::hyperprocess;

#[derive(Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct HyprEchoState {}

#[derive(Serialize, Deserialize, Debug)]
pub struct HyprEchoReq {
    payload: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HyprEchoResp {
    payload: String,
}

#[hyperprocess(
    name = "HyprEcho",
    ui = None,
    endpoints = vec![
        Binding::Http {
            path: "/api",
            config: HttpBindingConfig::new(false, false, false, None),
        },
        Binding::Ws {
            path: "/ws",
            config: WsBindingConfig::new(false, false, false),
        }
    ],
    save_config = SaveOptions::Never, // Changed as state is removed
    wit_world = "hypr-echo-template-dot-os-v0"
)]


impl HyprEchoState {
    // Initialize the process, every application needs an init function
    #[init]
    async fn initialize(&mut self) {
        println!("init HyprEcho");
    }

// Endpoint accepting both local, remote Hyperware requests, and HTTP requests
#[local]
#[remote]
#[http]
async fn echo(&self, req: HyprEchoReq) -> HyprEchoResp {
    // Print the received request, similar to the example snippet
    println!("got {:?}", req);
    // Return a fixed "Ack" response, mapped to the HyprEchoResp structure
    HyprEchoResp { payload: "Ack".to_string() }
}

    // Endpoint accepting WebSocket requests
    #[ws]
    fn ws_echo(&mut self, channel_id: u32, message_type: WsMessageType, blob: LazyLoadBlob) {
        println!("got: type={:?}, blob={:?}", message_type, blob);
        // Respond with "echo"
        send_ws_push(
            channel_id,
            WsMessageType::Text,
            LazyLoadBlob {
                mime: Some("application/json".to_string()),
                bytes: serde_json::to_vec("Ack").unwrap(),
            },
        );
    }
}
