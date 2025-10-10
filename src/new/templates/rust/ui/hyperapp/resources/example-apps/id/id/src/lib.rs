use caller_utils::sign::{sign_local_rpc, verify_local_rpc};
use hyperprocess_macro::hyperprocess;
use hyperware_process_lib::logging::{init_logging, Level};
use hyperware_process_lib::Address;

#[derive(Default, Debug, serde::Serialize, serde::Deserialize)]
struct IdState {}

fn make_sign_sys() -> Address {
    Address::new("our", ("sign", "sign", "sys"))
}

#[hyperprocess(
    name = "id",
    ui = Some(HttpBindingConfig::default()),
    endpoints = vec![
        Binding::Http {
            path: "/api",
            config: HttpBindingConfig::default(),
        },
        Binding::Ws {
            path: "/ws",
            config: WsBindingConfig::default(),
        }
    ],
    save_config = SaveOptions::Never,
    wit_world = "id-sys-v0",
)]
impl IdState {
    #[init]
    async fn init(&mut self) {
        init_logging(Level::DEBUG, Level::INFO, None, None, None).unwrap();
    }

    #[http]
    async fn sign(&mut self, message: Vec<u8>) -> Result<Vec<u8>, String> {
        let target = make_sign_sys();
        match sign_local_rpc(&target, message).await {
            Ok(r) => r,
            Err(e) => Err(e.to_string()),
        }
    }

    #[http]
    async fn verify(&mut self, message: Vec<u8>, signature: Vec<u8>) -> Result<bool, String> {
        let target = make_sign_sys();
        match verify_local_rpc(&target, message, signature).await {
            Ok(r) => r,
            Err(e) => Err(e.to_string()),
        }
    }
}
