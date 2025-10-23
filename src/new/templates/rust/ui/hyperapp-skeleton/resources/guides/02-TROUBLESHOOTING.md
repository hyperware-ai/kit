# 🔧 Comprehensive Troubleshooting Guide

## Quick Diagnosis Flow
```
┌─────────────────────┐
│   Build Failed?     │──Yes──> Check Section 1: Build Errors
└──────────┬──────────┘
           │ No
           ▼
┌─────────────────────┐
│  UI Shows "Node     │──Yes──> Check Section 2: Runtime Errors
│  Not Connected"?    │
└──────────┬──────────┘
           │ No
           ▼
┌─────────────────────┐
│  P2P Calls         │──Yes──> Check Section 3: P2P Issues
│  Failing?          │
└──────────┬──────────┘
           │ No
           ▼
┌─────────────────────┐
│  State/Data        │──Yes──> Check Section 4: State Issues
│  Problems?         │
└─────────────────────┘
```

---

## 1. Build Errors

### ❌ Error: "base64ct requires Rust 1.85" or edition2024 issues

**Symptoms:**
```
error: failed to parse manifest at `/Users/.../.cargo/registry/src/index.crates.io-6f17d22bba15001f/base64ct-1.8.0/Cargo.toml`
feature `edition2024` is required
```

**Root Cause:** Newer versions of base64ct require Rust edition 2024 which isn't stable yet

**Solution:** Pin base64ct to version 1.6.0 in your Cargo.toml
```toml
[dependencies]
# ... other dependencies ...
base64ct = "=1.6.0"  # Pin to avoid edition2024 requirement
```

### ❌ Error: "Failed to deserialize HTTP request"

**Symptoms:**
```
Failed to deserialize HTTP request: invalid type: unit variant, expected struct variant
```

**Root Cause:** HTTP endpoint parameter mismatch with caller-utils expectations

**Solutions:**

```rust
// ✅ Modern approach - Direct type deserialization (with generated caller-utils)
#[http(method = "POST")]
async fn create_item(&mut self, request: CreateItemReq) -> Result<ItemInfo, String> {
    // Process request directly
}

// ✅ Legacy approach - Manual JSON parsing
#[http]
async fn create_item(&mut self, request_body: String) -> Result<String, String> {
    let req: CreateRequest = serde_json::from_str(&request_body)?;
    // Process request...
}

// ❌ OLD GUIDANCE (may not work with newer versions)
#[http]
async fn get_status(&self, _request_body: String) -> StatusResponse {
    StatusResponse { ... }
}
```

**Note**: The modern approach requires TypeScript caller-utils that wrap requests properly.

### ❌ Error: "hyperware_process_lib is ambiguous"

**Full Error:**
```
error[E0659]: `hyperware_process_lib` is ambiguous
  --> src/lib.rs:2:5
   |
2  | use hyperware_process_lib::{our, homepage::add_to_homepage};
   |     ^^^^^^^^^^^^^^^^^^^^^ ambiguous name
```

**Root Cause:** `hyperware_process_lib` added to Cargo.toml dependencies

**Solution:**
```toml
# ❌ WRONG Cargo.toml
[dependencies]
hyperware_process_lib = "0.1"  # REMOVE THIS LINE!

# ✅ CORRECT Cargo.toml
[dependencies]
anyhow = "1.0"
process_macros = "0.1"
serde = { version = "1.0", features = ["derive"] }
# DO NOT add hyperware_process_lib - it's provided by the macro
```

### ❌ Error: "Found types used... that are neither WIT built-ins nor defined locally"

**Example:**
```
Found types used (directly or indirectly) in function signatures that are neither WIT built-ins nor defined locally: ["complex-data"]
```

**Root Causes & Solutions:**

1. **Using unsupported types:**
```rust
// ❌ WRONG - HashMap not supported
#[http]
async fn get_data(&self, _request_body: String) -> HashMap<String, Value> {
    self.data.clone()
}

// ✅ FIX 1: Use Vec<(K,V)>
#[http]
async fn get_data(&self, _request_body: String) -> Vec<(String, Value)> {
    self.data.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

// ✅ FIX 2: Return as JSON string
#[http]
async fn get_data(&self, _request_body: String) -> String {
    serde_json::to_string(&self.data).unwrap()
}
```

2. **Type not referenced in any handler:**
```rust
// ❌ PROBLEM: NestedType only used inside ResponseData
pub struct ResponseData {
    nested: NestedType,  // WIT won't find NestedType!
}

pub struct NestedType {
    value: String,
}

// ✅ FIX: Add dummy endpoint to expose the type
#[http]
async fn get_nested_type(&self, _request_body: String) -> NestedType {
    NestedType { value: "dummy".to_string() }
}
```

3. **Complex enums:**
```rust
// ❌ WRONG - Complex enum variants
pub enum Event {
    Created { id: String, data: Data },
    Updated { id: String, changes: Vec<Change> },
}

// ✅ FIX: Use simple enum + separate struct
pub enum EventType {
    Created,
    Updated,
}

pub struct EventData {
    event_type: EventType,
    id: String,
    data: Option<Data>,
    changes: Option<Vec<Change>>,
}
```

### ❌ Error: "the trait bound 'YourType: PartialEq' is not satisfied"

**Solution:**
```rust
// ❌ Missing PartialEq
#[derive(Serialize, Deserialize)]
pub struct MyType {
    field: String,
}

// ✅ Add PartialEq to derives
#[derive(Serialize, Deserialize, PartialEq)]
pub struct MyType {
    field: String,
}
```

### ❌ Error: "Method 'helper_function' in #[hyperprocess] impl block is missing required attribute"

**Root Cause:** All methods in `#[hyperprocess]` impl must have handler attributes

**Solution:**
```rust
// ❌ WRONG - Helper method in hyperprocess impl
#[hyperprocess(...)]
impl AppState {
    #[http]
    async fn endpoint(&mut self, _request_body: String) -> String {
        self.helper()  // Error!
    }
    
    fn helper(&self) -> String {  // Missing attribute!
        "data".to_string()
    }
}

// ✅ FIX: Move helpers to separate impl block
#[hyperprocess(...)]
impl AppState {
    #[http]
    async fn endpoint(&mut self, _request_body: String) -> String {
        self.helper()
    }
}

// Separate impl block for helpers
impl AppState {
    fn helper(&self) -> String {
        "data".to_string()
    }
}
```

---

## 2. Runtime Errors

### ❌ Error: "Node not connected" / "Your ID: Unknown"

**Symptoms:**
- UI shows "Node not connected"
- `window.our` is undefined
- WebSocket fails to connect

**Root Cause:** Missing `/our.js` script in HTML

**Solution:**
```html
<!-- ❌ WRONG - Missing script -->
<head>
    <meta charset="UTF-8" />
    <title>My App</title>
</head>

<!-- ✅ CORRECT - Script must be FIRST -->
<head>
    <script src="/our.js"></script>  <!-- MUST BE FIRST! -->
    <meta charset="UTF-8" />
    <title>My App</title>
</head>
```

**Debug in Browser Console:**
```javascript
// Check if script loaded
console.log(window.our);
// Should show: { node: "yournode.os", process: "app:package:publisher" }

// If undefined, check network tab for /our.js request
```

### ❌ Error: "Failed to parse ProcessId"

**Examples:**
```
Failed to parse ProcessId: InvalidFormat
```

**Root Cause:** Incorrect ProcessId format

**Solution:**
```rust
// ❌ WRONG formats
let pid = "myapp".parse::<ProcessId>();  // Missing parts
let pid = "myapp:myapp".parse::<ProcessId>();  // Missing publisher
let pid = "myapp-myapp-publisher".parse::<ProcessId>();  // Wrong separator

// ✅ CORRECT format: "process:package:publisher"
let pid = "myapp:myapp:publisher.os".parse::<ProcessId>()?;

// For your app matching remote nodes
let publisher = "skeleton.os";  // Or whatever the remote uses
let pid = format!("skeleton-app:skeleton-app:{}", publisher)
    .parse::<ProcessId>()?;
```

### ❌ Error: Parameter format mismatch

**Symptoms:** Frontend call succeeds but backend receives wrong data

**Root Cause:** Multi-parameter endpoints need tuple format

**Solution:**
```typescript
// ❌ WRONG - Object format
const response = await fetch('/api', {
    body: JSON.stringify({
        CreateItem: {
            name: "Item",
            description: "Description"
        }
    })
});

// ✅ CORRECT - Tuple/array format for multiple params
const response = await fetch('/api', {
    body: JSON.stringify({
        CreateItem: ["Item", "Description"]
    })
});

// For single parameter, value directly
const response = await fetch('/api', {
    body: JSON.stringify({
        GetItem: "item-id-123"
    })
});
```

---

## 3. P2P Communication Issues

### ❌ Error: "SendError" or "Failed to send request"

**Common Causes:**

1. **Target node not running:**
```bash
# Check if target node is accessible
# In your node's terminal, you should see incoming requests
```

2. **Wrong node name:**
```rust
// ❌ WRONG - Using placeholder
let target = Address::new("placeholder.os", process_id);

// ✅ CORRECT - Use actual node name
let target = Address::new("alice.os", process_id);  // Real node
```

3. **Missing timeout:**
```rust
// ❌ WRONG - No timeout set
Request::new()
    .target(address)
    .body(data)
    .send();

// ✅ CORRECT - Always set expects_response
Request::new()
    .target(address)
    .body(data)
    .expects_response(30)  // REQUIRED!
    .send_and_await_response(30)?;
```

4. **Wrong request format:**
```rust
// ❌ WRONG - Array format
let wrapper = json!({
    "HandleRequest": [param1, param2]  // Arrays don't work
});

// ✅ CORRECT - Tuple format for multiple params
let wrapper = json!({
    "HandleRequest": (param1, param2)  // Tuple format
});

// ✅ CORRECT - Single param
let wrapper = json!({
    "HandleRequest": param
});
```

### ❌ Error: Remote endpoint not found

**Symptom:** Call succeeds but returns error about missing method

**Root Cause:** Method name mismatch or missing #[remote] attribute

**Solution:**
```rust
// On receiving node:
#[remote]  // Must have this attribute!
async fn handle_sync(&mut self, data: String) -> Result<String, String> {
    // Implementation
}

// On calling node:
let wrapper = json!({
    "HandleSync": data  // Must match exactly (case-sensitive)
});
```

### ❌ Error: Can't decode remote response

**Root Cause:** Response type mismatch

**Solution:**
```rust
// ❌ Expecting wrong type
let response: ComplexType = serde_json::from_slice(&response.body())?;

// ✅ Match what remote actually returns
let response: String = serde_json::from_slice(&response.body())?;
// Then parse if needed
let data: ComplexType = serde_json::from_str(&response)?;
```

### ❌ Error: ProcessId parse errors in P2P apps

**Symptoms:**
```
Failed to parse ProcessId: InvalidFormat
```

**Common P2P Pattern:**
```rust
// ❌ WRONG - Hardcoded publisher assumption
let pid = "samchat:samchat:publisher.os".parse::<ProcessId>()?;

// ✅ CORRECT - Use consistent publisher across nodes
let publisher = "hpn-testing-beta.os"; // Or get from config
let target_process_id_str = format!("samchat:samchat:{}", publisher);
let target_process_id = target_process_id_str.parse::<ProcessId>()
    .map_err(|e| format!("Failed to parse ProcessId: {}", e))?;
```

### ❌ Error: Node ID not initialized

**Symptoms:**
```
Sender node ID not initialized
```

**Root Cause:** Trying to use node ID before init

**Solution:**
```rust
// In state
pub struct AppState {
    my_node_id: Option<String>,
}

// In init
#[init]
async fn initialize(&mut self) {
    self.my_node_id = Some(our().node.clone());
}

// In handlers
let sender = self.my_node_id.clone()
    .ok_or_else(|| "Node ID not initialized".to_string())?;
```

### ❌ Error: Group/conversation management issues

**Common P2P Chat Errors:**
```rust
// Group not found
let conversation = self.conversations.get(&group_id)
    .ok_or_else(|| "Group conversation not found".to_string())?;

// Not a group conversation
if !conversation.is_group {
    return Err("Not a group conversation".to_string());
}

// Member already exists
if conversation.participants.contains(&new_member) {
    return Err("Member already in group".to_string());
}
```

### ❌ Error: Remote file/data fetch failures

**Complex P2P data retrieval pattern:**
```rust
// Try local first, then remote
match local_result {
    Ok(response) => {
        if let Some(blob) = response.blob() {
            return Ok(blob.bytes);
        }
    },
    Err(_) => {
        // Fetch from remote node
        let remote_result = Request::new()
            .target(remote_address)
            .body(request_body)
            .expects_response(30)
            .send_and_await_response(30)?;
            
        match remote_result {
            Ok(response) => {
                // Parse nested Result
                let response_json: serde_json::Value = 
                    serde_json::from_slice(&response.body())?;
                
                if let Some(data) = response_json.get("Ok") {
                    // Handle success
                } else if let Some(err) = response_json.get("Err") {
                    return Err(format!("Remote error: {}", err));
                }
            },
            Err(e) => return Err(format!("Remote fetch failed: {:?}", e))
        }
    }
}
```

---

## 4. State Management Issues

### ❌ Error: State not persisting

**Root Cause:** Wrong save_config or state not serializable

**Solution:**
```rust
#[hyperprocess(
    // ...
    save_config = SaveOptions::EveryMessage,  // Most reliable
    // OR
    save_config = SaveOptions::OnInterval(30),  // Every 30 seconds
)]

// Ensure state is serializable
#[derive(Default, Serialize, Deserialize)]
pub struct AppState {
    // All fields must be serializable
}
```

### ❌ Error: Race conditions in React state

**Symptom:** Action uses old state value

**Solution:**
```typescript
// ❌ WRONG - State might not be updated
const handleJoin = async (gameId: string) => {
    setSelectedGame(gameId);
    await joinGame();  // Uses selectedGame from state - WRONG!
};

// ✅ CORRECT - Pass value explicitly
const handleJoin = async (gameId: string) => {
    setSelectedGame(gameId);
    await joinGame(gameId);  // Pass directly
};

// ✅ BETTER - Use callback form
const handleUpdate = () => {
    setItems(prevItems => {
        // Work with prevItems, not items from closure
        return [...prevItems, newItem];
    });
};
```

### ❌ Error: Stale data in UI

**Root Cause:** Not refreshing after mutations

**Solution:**
```typescript
// In your store
const createItem = async (data: CreateData) => {
    try {
        await api.createItem(data);
        // ✅ Refresh data after mutation
        await get().fetchItems();
    } catch (error) {
        // Handle error
    }
};

// With optimistic updates
const deleteItem = async (id: string) => {
    // Optimistic update
    set(state => ({
        items: state.items.filter(item => item.id !== id)
    }));
    
    try {
        await api.deleteItem(id);
    } catch (error) {
        // Rollback on error
        await get().fetchItems();
        throw error;
    }
};
```

---

## 5. Manifest & Capability Issues

### ❌ Error: "failed to open file `pkg/manifest.json`"

**Full Error:**
```
ERROR: failed to open file `/path/to/app/pkg/manifest.json`
No such file or directory (os error 2)
```

**Root Cause:** manifest.json not generated during build

**Solutions:**

1. **Build properly with kit:**
```bash
# This generates manifest.json automatically
kit b --hyperapp
```

2. **Check if pkg directory exists:**
```bash
ls -la pkg/
# Should contain: manifest.json, your-app.wasm, ui/
```

3. **If still missing, check metadata.json:**
```json
// metadata.json must exist and be valid
{
  "package": "skeleton-app",
  "publisher": "skeleton.os"
}
```

**See**: [Manifest & Deployment Guide](./08-MANIFEST-AND-DEPLOYMENT.md) for details

### ❌ Error: "Process does not have capability X"

**Example:**
```
Error: Process skeleton-app:skeleton-app:user.os does not have capability vfs:distro:sys
```

**Root Cause:** Using system feature without requesting capability

**Solution:** Add to manifest.json:
```json
"request_capabilities": [
  "homepage:homepage:sys",
  "http-server:distro:sys",
  "vfs:distro:sys"  // Add missing capability
]
```

**See**: [Capabilities Guide](./09-CAPABILITIES-GUIDE.md) for all capabilities

### ❌ Error: App doesn't appear on homepage

**Root Cause:** Missing homepage capability or add_to_homepage call

**Solution:**
1. Check manifest.json includes:
```json
"request_capabilities": [
  "homepage:homepage:sys"  // Required!
]
```

2. Check init function calls:
```rust
#[init]
async fn initialize(&mut self) {
    add_to_homepage("My App", Some("🚀"), Some("/"), None);
}
```

---

## 6. Development Workflow Issues

### Clean Build Process
```bash
# When things are really broken
rm -rf target/
rm -rf ui/node_modules ui/dist
rm -rf pkg/
rm Cargo.lock

# Fresh build
kit b --hyperapp
```

### Check Generated Files
```bash
# View generated WIT
cat api/*.wit

# Check built package
ls -la pkg/

# Verify UI was built
ls -la pkg/ui/
```

### Test Incrementally
```bash
# 1. Test backend compiles
cd skeleton-app && cargo check

# 2. Test UI builds
cd ui && npm run build

# 3. Full build
cd .. && kit b --hyperapp
```

---

## 6. Common Patterns That Cause Issues

### ❌ WebSocket Handler Issues
```rust
// ❌ WRONG - Async WebSocket handler
#[ws]
async fn websocket(&mut self, channel_id: u32, message_type: WsMessageType, blob: LazyLoadBlob) {
    // WebSocket handlers must NOT be async!
}

// ✅ CORRECT - Synchronous handler
#[ws]
fn websocket(&mut self, channel_id: u32, message_type: WsMessageType, blob: LazyLoadBlob) {
    match message_type {
        WsMessageType::Text => {
            // Handle text message
        }
        WsMessageType::Close => {
            // Handle disconnect
        }
        _ => {}
    }
}
```

**Common WebSocket Issues:**
1. **Missing endpoint configuration** in hyperprocess macro:
```rust
#[hyperprocess(
    endpoints = vec![
        Binding::Ws {
            path: "/ws",
            config: WsBindingConfig::default().authenticated(false),
        },
    ],
)]
```

2. **Frontend connection issues:**
```typescript
// ❌ WRONG - Missing authentication
const ws = new WebSocket('ws://localhost:8080/ws');

// ✅ CORRECT - Include proper URL
const ws = new WebSocket(`ws://${window.location.host}/${appName}/ws`);
```

### ❌ Forgetting async on endpoints
```rust
// ❌ WRONG - Not async
#[http]
fn get_data(&self, _request_body: String) -> String {
    // Won't compile
}

// ✅ CORRECT - Must be async
#[http]
async fn get_data(&self, _request_body: String) -> String {
    // Works
}
```

### ❌ Wrong imports order
```rust
// ❌ Can cause issues
use serde::{Serialize, Deserialize};
use hyperprocess_macro::*;

// ✅ Better order
use hyperprocess_macro::*;
use hyperware_process_lib::{our, Address, ProcessId, Request};
use serde::{Deserialize, Serialize};
```

---

## Debug Checklist

When nothing works, check:

1. **Build issues:**
   - [ ] All HTTP methods have `_request_body` parameter?
   - [ ] No `hyperware_process_lib` in Cargo.toml?
   - [ ] All types are WIT-compatible?
   - [ ] `#[hyperprocess]` before impl block?

2. **Runtime issues:**
   - [ ] `/our.js` script in HTML head?
   - [ ] Node is actually running?
   - [ ] Correct ProcessId format?
   - [ ] Frontend using tuple format for params?

3. **P2P issues:**
   - [ ] Target node running?
   - [ ] Using real node names?
   - [ ] `expects_response` timeout set?
   - [ ] Method names match exactly?

4. **State issues:**
   - [ ] State is serializable?
   - [ ] Refreshing after mutations?
   - [ ] Passing values explicitly (not from React state)?

## 7. Audio/Real-time Data Issues (Voice Apps)

### ❌ Base64 encoding/decoding issues
```rust
// ❌ WRONG - Manual base64 handling
let decoded = base64::decode(&data)?;

// ✅ CORRECT - Use proper engine
use base64::{Engine as _, engine::general_purpose};
let decoded = general_purpose::STANDARD.decode(&data).unwrap_or_default();
let encoded = general_purpose::STANDARD.encode(&bytes);
```

### ❌ Thread safety with audio processing
```rust
// ❌ WRONG - Direct mutation in WebSocket handler
self.audio_buffer.push(audio_data);

// ✅ CORRECT - Use Arc<Mutex<>> for thread-safe access
use std::sync::{Arc, Mutex};

// In state
audio_processors: HashMap<String, Arc<Mutex<AudioProcessor>>>,

// In handler
if let Ok(mut proc) = processor.lock() {
    proc.process_audio(data);
}
```

### ❌ WebSocket message sequencing
```rust
// Track sequence numbers for audio streams
#[derive(Serialize, Deserialize)]
struct AudioData {
    data: String,
    sequence: Option<u32>,
    timestamp: Option<u64>,
}

// Maintain sequence counters
participant_sequences: HashMap<String, u32>,
```

### ❌ Binary data in LazyLoadBlob
```rust
// For binary WebSocket data
let blob = LazyLoadBlob {
    mime: Some("application/octet-stream".to_string()),
    bytes: audio_bytes,
};
send_ws_push(channel_id, WsMessageType::Binary, blob);
```

## 8. P2P Validation Patterns

### Common P2P validation errors from samchat:

**Backend validation:**
```rust
// Empty fields
if recipient_address.trim().is_empty() || message_content.trim().is_empty() {
    return Err("Recipient address and message content cannot be empty".to_string());
}

// Format validation
if !is_group && !recipient_address.contains('.') {
    return Err("Invalid recipient address format (e.g., 'username.os')".to_string());
}

// Group constraints
if participants.len() < 2 {
    return Err("Group must have at least 2 participants".to_string());
}
```

**Frontend validation:**
```typescript
// In React component
if (!groupName.trim()) {
    setError("Please enter a group name");
    return;
}

// Parse and validate lists
const members = groupMembers.split(',').map(m => m.trim()).filter(m => m);
if (members.length === 0) {
    setError("Please enter at least one valid member address");
    return;
}

// Clear errors on navigation
const handleSelectConversation = useCallback((conversationId: string) => {
    fetchMessages(conversationId);
    setError(null);
    setReplyingTo(null);
}, [fetchMessages]);
```

## Still Stuck?

1. Add logging everywhere:
   ```rust
   println!("DEBUG: Method called with: {:?}", request_body);
   ```

2. Check both node consoles for P2P issues

3. Use browser DevTools:
   - Network tab for HTTP/WebSocket
   - Console for JavaScript errors
   - Application tab for storage

4. For voice apps:
   - Check browser permissions for microphone
   - Monitor WebSocket frames in DevTools
   - Log audio buffer sizes and timing

5. Start with minimal example and add complexity

6. Compare with working examples:
   - samchat for P2P chat patterns
   - voice for WebSocket/audio patterns