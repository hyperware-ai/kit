use serde::{Serialize, Deserialize};
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
    response: String,
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
    }

    // Endpoint accepting both local, remote Hyperware requests, and HTTP requests
    #[local]
    #[remote]
    #[http]
    async fn echo(&self, arg: Argument) -> ReturnValue {
        println!("header: {:?}, body: {:?}", arg.header, arg.body);

        ReturnValue { response: "Ack".to_string() }
    }

}
