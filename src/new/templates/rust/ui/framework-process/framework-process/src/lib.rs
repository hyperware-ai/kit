use hyperprocess_macro::hyperprocess;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// --- State ---
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct FrameworkProcessState {
    state: HashMap<String, String>,
}

// --- Hyperware Process ---
#[hyperprocess(
    name = "FrameworkProcess",
    ui = Some(HttpBindingConfig::default()),
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
    save_config = SaveOptions::EveryMessage,
    wit_world = "framework-process-template-dot-os-v0"
)]

// --- Hyperware Process API definitions ---
impl FrameworkProcessState {
    #[init]
    async fn initialize(&mut self) {
        println!("init");
        self.state = HashMap::new();
    }

    // Local Hyperware request
    #[local]
    async fn add_to_state(&mut self, value: String) -> Result<bool, String> {
        self.state.insert(value.clone(), value);
        Ok(true)
    }

    // Double annotation for endpoint accepting both local and remote Hyperware requests
    // Can also add #[http] as a third annotation to extend the endpoint to HTTP requests
    #[local]
    #[remote]
    async fn get_state(&self) -> Result<Vec<String>, String> {
        Ok(self.state.values().cloned().collect())
    }

    // HTTP endpoint, will need to be a POST request on the frontend
    // to the /api endpoint
    // We add an empty string as a parameter to satisfy the HTTP POST
    // requirement, but it is not used.
    #[http]
    async fn view_state(&self, empty: String) -> Result<Vec<String>, String> {
        println!("Payload: {}", empty);
        Ok(self.state.values().cloned().collect())
    }

    // HTTP endpoint, will need to be a POST request on the frontend
    // to the /api endpoint
    #[http]
    async fn submit_entry(&mut self, value: String) -> Result<bool, String> {
        self.state.insert(value.clone(), value);
        Ok(true)
    }
}
