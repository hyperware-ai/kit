// HYPERWARE SKELETON APP
// This is a minimal, well-commented skeleton app for the Hyperware platform
// using the Hyperapp framework (macro-driven approach).

// CRITICAL IMPORTS - DO NOT MODIFY THESE
// The hyperprocess_macro provides everything you need including:
// - Async/await support (custom runtime)
// - Automatic WIT (WebAssembly Interface Types) generation
// - State persistence
// - HTTP/WebSocket bindings
use hyperprocess_macro::hyperprocess;

// HYPERWARE PROCESS LIB IMPORTS
// These are provided by the hyperprocess_macro, DO NOT add hyperware_process_lib to Cargo.toml
use hyperware_process_lib::{homepage::add_to_homepage, our, println};

// Standard imports for serialization
use serde::{Deserialize, Serialize};

// STEP 1: DEFINE YOUR APP STATE
// This struct holds all persistent data for your app
// It MUST derive Default, Serialize, and Deserialize
// Add PartialEq if you use this type in WIT interfaces
#[derive(Default, Serialize, Deserialize)]
pub struct AppState {
    // Example fields - replace with your app's data
    counter: u32,
    messages: Vec<String>,
}

// STEP 2: IMPLEMENT YOUR APP LOGIC
// The #[hyperprocess] attribute goes HERE, before the impl block
#[hyperprocess(
    name = "Skeleton App",
    ui = Some(hyperware_process_lib::http::server::HttpBindingConfig::default()),
    endpoints = vec![
        hyperware_process_lib::hyperapp::Binding::Http {
            path: "/api",
            config: hyperware_process_lib::http::server::HttpBindingConfig::new(false, false, false, None),
        },
    ],
    save_config = hyperware_process_lib::hyperapp::SaveOptions::EveryMessage,
    wit_world = "skeleton-app-dot-os-v0"
)]
impl AppState {
    // INITIALIZATION FUNCTION
    // Runs once when your process starts
    // Use this to:
    // - Register with the homepage
    // - Set up initial state
    // - Connect to other system processes
    #[init]
    async fn initialize(&mut self) {
        // Add your app to the Hyperware homepage
        // Parameters: name, icon (emoji), path, widget
        add_to_homepage("Skeleton App", Some("🦴"), Some("/"), None);
        
        // Initialize your app state
        self.counter = 0;
        self.messages.push("App initialized!".to_string());
        
        // Get our node identity (useful for P2P apps)
        let our_node = our().node.clone();
        println!("Skeleton app initialized on node: {}", our_node);
    }
    
    // HTTP ENDPOINT EXAMPLE
    // CRITICAL: ALL HTTP endpoints MUST accept _request_body parameter
    // even if they don't use it. This is a framework requirement.
    #[http]
    async fn get_status(&self, _request_body: String) -> String {
        // Return current app status as JSON
        serde_json::json!({
            "counter": self.counter,
            "message_count": self.messages.len(),
            "node": our().node
        }).to_string()
    }
    
    // HTTP ENDPOINT WITH PARAMETERS
    // Frontend sends parameters as either:
    // - Single value: { "MethodName": value }
    // - Multiple values as tuple: { "MethodName": [val1, val2] }
    #[http]
    async fn increment_counter(&mut self, request_body: String) -> Result<u32, String> {
        // Parse the increment amount from request
        let amount: u32 = match serde_json::from_str(&request_body) {
            Ok(val) => val,
            Err(_) => 1, // Default increment
        };
        
        self.counter += amount;
        self.messages.push(format!("Counter incremented by {}", amount));
        
        Ok(self.counter)
    }
    
    // HTTP ENDPOINT RETURNING COMPLEX DATA
    // For complex types, return as JSON string to avoid WIT limitations
    #[http]
    async fn get_messages(&self, _request_body: String) -> String {
        serde_json::to_string(&self.messages).unwrap_or_else(|_| "[]".to_string())
    }
    
}


// WIT TYPE COMPATIBILITY NOTES:
// The hyperprocess macro generates WebAssembly Interface Types from your code.
// Supported types:
// ✅ Primitives: bool, u8-u64, i8-i64, f32, f64, String
// ✅ Vec<T> where T is supported
// ✅ Option<T> where T is supported  
// ✅ Simple structs with public fields
// ❌ HashMap - use Vec<(K,V)> instead
// ❌ Fixed arrays [T; N] - use Vec<T>
// ❌ Complex enums with data
// 
// Workaround: Return complex data as JSON strings

// COMMON PATTERNS:

// 1. STATE MANAGEMENT
// Your AppState is automatically persisted based on save_config
// Access current state with &self (read) or &mut self (write)

// 2. ERROR HANDLING
// Return Result<T, String> for fallible operations
// The String error will be sent to the frontend

// 3. FRONTEND COMMUNICATION
// Frontend calls HTTP endpoints via POST to /api
// Body format: { "MethodName": parameters }

// 4. P2P PATTERNS
// - See the P2P patterns guide for implementing P2P features
// - Use #[remote] for methods other nodes can call
// - Use Request API for calling other nodes
// - Always set timeouts for remote calls

// 5. SYSTEM INTEGRATION
// Common system processes you might interact with:
// - "vfs:distro:sys" - Virtual file system
// - "http-server:distro:sys" - HTTP server (automatic with macro)
// - "timer:distro:sys" - Timers and scheduling
// - "kv:distro:sys" - Key-value storage

// DEVELOPMENT WORKFLOW:
// 1. Define your AppState structure
// 2. Add HTTP endpoints for UI interaction
// 3. Add remote endpoints for P2P features
// 4. Build with: kit b --hyperapp
// 5. Start with: kit s
// 6. Access at: http://localhost:8080

// DEBUGGING TIPS:
// - Use println! for backend logs (appears in terminal)
// - Check browser console for frontend errors
// - Common issues:
//   * Missing _request_body parameter
//   * Wrong parameter format (object vs tuple)
//   * ProcessId parsing errors
//   * Missing /our.js in HTML
