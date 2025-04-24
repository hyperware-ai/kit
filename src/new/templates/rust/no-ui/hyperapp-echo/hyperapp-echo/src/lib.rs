use serde::{Serialize, Deserialize};
use hyperware_process_lib::{
    LazyLoadBlob,
    http::server::{WsMessageType, send_ws_push},
    homepage::add_to_homepage,
};
use hyperprocess_macro::hyperprocess;


#[derive(Default, Debug, Serialize, Deserialize)]
pub struct HyperappEchoState {}

#[derive(Serialize, Deserialize, Debug)]
pub struct Argument {
    header: String,
    body: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ReturnValue {
    result: String,
}

#[hyperprocess(
    name = "HyperappEcho",
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
    save_config = SaveOptions::Never,
    wit_world = "hyperapp-echo-template-dot-os-v0"
)]

impl HyperappEchoState {
    // Initialize the process, every application needs an init function
    #[init]
    async fn initialize(&mut self) {
        println!("init HyperappEcho");
        add_to_homepage("HyperappEcho", Some(ICON), Some(""), None);
    }

    // Endpoint accepting both local, remote Hyperware requests, and HTTP requests
    #[local]
    #[remote]
    #[http]
    async fn echo(&self, arg: Argument) -> ReturnValue {
        println!("header: {:?}, body: {:?}", arg.header, arg.body);

        ReturnValue { result: "Ack".to_string() }
    }

    // Endpoint accepting WebSocket requests
    #[ws]
    async fn ws_echo(&mut self, channel_id: u32, message_type: WsMessageType, blob: LazyLoadBlob) {
        println!("got: type={:?}, blob={:?}", message_type, blob);

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
