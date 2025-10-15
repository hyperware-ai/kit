# 🔐 Capabilities System Guide

## What Are Capabilities?

Capabilities are Hyperware's security system. Think of them as permissions that control what your app can do. Without the right capabilities, your app can't:
- Access the filesystem
- Make HTTP requests  
- Show on the homepage
- Use databases
- Interact with Ethereum

**IMPORTANT**: Capabilities must be requested in `manifest.json` BEFORE your app tries to use them.

## Core System Capabilities

### 📍 homepage:homepage:sys
**Purpose**: Add your app icon to the Hyperware homepage

**Used for**:
```rust
add_to_homepage("My App", Some("🚀"), Some("/"), None);
```

**Without it**: Your app runs but users can't find it!

---

### 🌐 http-client:distro:sys
**Purpose**: Make outbound HTTP requests to external services

**Used for**:
- Fetching data from external APIs
- Webhooks
- OAuth flows
- RSS feeds
- Calling other nodes' HTTP endpoints (alternative to native P2P)

**Example**:
```rust
use hyperware_process_lib::http::{ClientRequest, Method};

let response = ClientRequest::new()
    .method(Method::Get)
    .url("https://api.example.com/data")
    .send()
    .await?;
```

**Note**: For P2P messaging between Hyperware processes, use the native Request API (requires `request_networking: true` in manifest.json), NOT http-client!

---

### 🖥️ http-server:distro:sys
**Purpose**: Serve HTTP endpoints and UI

**Required for**:
- The `/api` endpoint for your frontend
- Serving your UI at root path
- WebSocket connections (`/ws` endpoints)
- Any HTTP endpoints
- Dynamic UI serving (e.g., `/call/<id>`)

**Without it**: No UI, no API!

**Note**: This single capability enables both HTTP and WebSocket support - no separate WebSocket capability needed!

---

### 📁 vfs:distro:sys
**Purpose**: Virtual filesystem access

**Operations**:
```rust
use hyperware_process_lib::vfs::*;

// Create directory
create_directory("/app-data", None)?;

// Write file
create_file("/app-data/config.json", None)?;
write_file("/app-data/config.json", config_bytes)?;

// Read file
let data = read_file("/app-data/config.json")?;

// List files
let entries = read_directory("/app-data")?;
```

**Use cases**:
- Store user uploads
- Cache data
- Configuration files
- Temporary files

---

### ⛓️ eth:distro:sys
**Purpose**: Ethereum blockchain interaction

**Features**:
- Read blockchain data
- Send transactions
- Interact with smart contracts
- Monitor events

**Example**:
```rust
use hyperware_process_lib::eth::*;

// Get ETH balance
let balance = get_balance(wallet_address).await?;

// Call smart contract
let result = call_contract(
    contract_address,
    "balanceOf",
    vec![user_address]
).await?;
```

---

### 💾 sqlite:distro:sys
**Purpose**: SQLite database access

**⚠️ IMPORTANT**: SQLite requires BOTH capabilities:
```json
"request_capabilities": [
  "sqlite:distro:sys",  // For database operations
  "vfs:distro:sys"      // Required! SQLite uses VFS internally
]
```

**Operations**:
```rust
use hyperware_process_lib::sqlite;

// Open database
let db = sqlite::open(our().package_id(), "my_database", Some(5000))?;

// Create tables
db.write(
    "CREATE TABLE IF NOT EXISTS users (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL
    )".to_string(),
    vec![],
    None
)?;

// Query data
let users = db.read("SELECT * FROM users".to_string(), vec![])?;
```

See the [SQLite API Guide](./10-SQLITE-API-GUIDE.md) for comprehensive usage.

---

### ⏰ timer:distro:sys
**Purpose**: Schedule delayed or recurring tasks

**Use cases**:
- Periodic data syncing
- Scheduled jobs
- Timeouts
- Delayed operations

**Example**:
```rust
use hyperware_process_lib::timer::*;

// One-time timer (5 seconds)
set_timer(5000);

// Handle in message loop
match message {
    Message::Timer(_) => {
        // Timer fired!
        self.sync_data().await?;
    }
}
```

---

### 🔑 kv:distro:sys
**Purpose**: Key-value store for simple persistence

**Operations**:
```rust
use hyperware_process_lib::kv::*;

// Set value
set("user:123", json!({"name": "Alice"}))?;

// Get value
let user = get("user:123")?;

// Delete
delete("user:123")?;

// List keys
let keys = list_keys("user:*")?;
```

**Best for**:
- Settings/preferences
- Simple caching
- Session storage

---

### 🌐 net:tcp:sys & net:udp:sys
**Purpose**: Raw TCP/UDP networking

**Note**: The exact capability format may vary. You might see:
- `net:tcp:sys` and `net:udp:sys` (specific protocols)
- `net:distro:sys` (general networking)
- Verify with your Hyperware version

**Use cases**:
- Custom protocols
- Game servers
- IoT communication
- Non-HTTP services

---

## Capability Request Patterns

### Basic App (UI + API)
```json
"request_capabilities": [
  "homepage:homepage:sys",
  "http-server:distro:sys"
]
```

### Data Storage App
```json
"request_capabilities": [
  "homepage:homepage:sys",
  "http-server:distro:sys",
  "vfs:distro:sys",
  "sqlite:distro:sys"
]
```

### External API Consumer
```json
"request_capabilities": [
  "homepage:homepage:sys",
  "http-server:distro:sys",
  "http-client:distro:sys",
  "timer:distro:sys"  // For periodic fetching
]
```

### DeFi/Web3 App
```json
"request_capabilities": [
  "homepage:homepage:sys",
  "http-server:distro:sys",
  "http-client:distro:sys",
  "eth:distro:sys"
]
```

### P2P Collaborative App (Chat, File Sharing, etc.)
```json
"request_networking": true,       // CRITICAL: Enables P2P messaging via Request API
"request_capabilities": [
  "homepage:homepage:sys",
  "http-server:distro:sys",       // Serve UI and API endpoints
  "vfs:distro:sys"                // Store messages, files, and state
]
```

**Note**: P2P messaging in Hyperware uses the native Request API, which requires `"request_networking": true` in manifest.json. The `http-client:distro:sys` capability is only needed for external HTTP requests, not for P2P!

### Real-time Voice/Video App
```json
"request_capabilities": [
  "homepage:homepage:sys",
  "http-server:distro:sys",  // Includes WebSocket support
  "vfs:distro:sys"           // For storing settings/history
]
```

## P2P Messaging vs HTTP Client

**IMPORTANT**: There are two ways to communicate between Hyperware nodes:

### 1. Native Request API (Recommended for P2P)
```rust
// Uses request_networking: true in manifest.json
Request::new()
    .target(Address::new(node_id, "process:package:publisher"))
    .body(message_bytes)
    .send_and_await_response(30)?;
```
- Requires: `"request_networking": true` in manifest.json
- Does NOT require: Any specific capability
- Used for: Direct process-to-process messaging
- More efficient for P2P apps

### 2. HTTP Client to Node Endpoints
```rust
// Uses http-client:distro:sys capability
ClientRequest::new()
    .url(&format!("http://{}/api/endpoint", node_address))
    .body(data)
    .send()
    .await?;
```
- Requires: `"http-client:distro:sys"` capability
- Used for: External APIs or when you need HTTP semantics
- Less efficient for P2P but works across different systems

## What Doesn't Need Capabilities?

Many browser APIs work without special Hyperware capabilities:

### ✅ Works Without Special Capabilities:
- **WebRTC** - Audio/video calls, screen sharing
- **getUserMedia** - Camera/microphone access
- **Web Audio API** - Audio processing, effects
- **Canvas/WebGL** - Graphics rendering
- **Geolocation** - Location services
- **IndexedDB** - Browser storage
- **Service Workers** - Offline functionality
- **Web Workers** - Background processing
- **Notifications API** - Browser notifications

These are browser-level APIs that work if the user grants browser permissions. They don't need Hyperware capabilities!

## Granting Capabilities

Your app can grant capabilities to specific processes:

### Allow Terminal to Message You
```json
"grant_capabilities": [
  {
    "process": "terminal:terminal:sys",
    "capabilities": ["messaging"]
  }
]
```

### Grant Multiple Capabilities
```json
"grant_capabilities": [
  {
    "process": "backup:backup:john.os",
    "capabilities": ["messaging", "read-file"]
  },
  {
    "process": "monitor:monitor:sys",
    "capabilities": ["messaging"]
  }
]
```

## Security Best Practices

### 1. Request Only What You Need
```json
// ❌ BAD - Requesting everything
"request_capabilities": [
  "homepage:homepage:sys",
  "http-client:distro:sys",
  "http-server:distro:sys",
  "vfs:distro:sys",
  "eth:distro:sys",
  "sqlite:distro:sys",
  "timer:distro:sys",
  "kv:distro:sys",
  "net:tcp:sys",
  "net:udp:sys"
]

// ✅ GOOD - Only what's used
"request_capabilities": [
  "homepage:homepage:sys",
  "http-server:distro:sys"
]
```

### 2. Document Why You Need Each Capability
```rust
// In your README or comments:
// - homepage:homepage:sys: Display app icon
// - http-server:distro:sys: Serve UI and API
// - vfs:distro:sys: Store user uploads
// - sqlite:distro:sys: User data persistence
```

### 3. Handle Missing Capabilities Gracefully
```rust
// Check if operation will work
match create_file("/data/test.txt", None) {
    Ok(_) => println!("VFS available"),
    Err(e) => {
        println!("VFS not available: {}", e);
        // Fallback to in-memory storage
    }
}
```

## Common Capability Errors

### ❌ "CapabilityNotFound"
```
Error: Process skeleton-app:skeleton-app:user.os does not have capability vfs:distro:sys
```
**Fix**: Add missing capability to manifest.json

### ❌ "Messaging permission denied"
```
Error: Process X cannot message process Y
```
**Fix**: Either:
- Make target process `"public": true`
- Or grant messaging capability to sender

### ❌ Feature silently fails
**Symptom**: Code runs but nothing happens
**Cause**: Missing capability (no error thrown)
**Fix**: Check all system calls have matching capabilities

## Capability Requirements by Feature

| Feature | Required Capabilities |
|---------|----------------------|
| Show app icon | `homepage:homepage:sys` |
| Serve UI | `http-server:distro:sys` |
| API endpoints | `http-server:distro:sys` |
| WebSocket endpoints | `http-server:distro:sys` |
| External APIs | `http-client:distro:sys` |
| File storage | `vfs:distro:sys` |
| Database | `sqlite:distro:sys` + `vfs:distro:sys` |
| Blockchain | `eth:distro:sys` |
| Scheduling | `timer:distro:sys` |
| Settings storage | `kv:distro:sys` |
| Custom networking | `net:tcp:sys`, `net:udp:sys` |
| WebRTC/Audio/Video | None (browser APIs) |

## Testing Capabilities

1. **Start minimal**: Only homepage + http-server
2. **Add as needed**: When you add features
3. **Test each**: Verify the feature works
4. **Document**: Note why each is needed

## Advanced: Custom Capabilities

For your own process-to-process permissions:

```rust
// Process A grants custom capability
"grant_capabilities": [
  {
    "process": "plugin:plugin:developer.os",
    "capabilities": ["read-data", "write-config"]
  }
]

// Process B checks before operations
if has_capability(sender, "write-config") {
    self.update_config(new_config)?;
}
```

## Summary

- **Capabilities = Permissions** for system resources
- **Request in manifest.json** before using
- **Start minimal**, add as needed
- **Document** why you need each one
- **Handle failures** gracefully
- **Test** with different capability sets

Remember: Users see requested capabilities during install. Request only what you truly need!