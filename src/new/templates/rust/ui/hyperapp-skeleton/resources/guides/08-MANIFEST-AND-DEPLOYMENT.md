# 📦 Manifest & Deployment Guide

## What is manifest.json?

The `manifest.json` file is **CRITICAL** for your Hyperware app to run. Without it, you'll get:
```
ERROR: failed to open file `pkg/manifest.json`
No such file or directory
```

This file tells Hyperware:
- How to identify your process
- What system capabilities it needs
- How to handle crashes
- Whether other processes can message it

## manifest.json Structure

### Basic Example (Skeleton App)
```json
[
  {
    "process_name": "skeleton-app",
    "process_wasm_path": "/skeleton-app.wasm",
    "on_exit": "Restart",
    "request_networking": true,
    "request_capabilities": [
      "homepage:homepage:sys",
      "http-server:distro:sys"
    ],
    "grant_capabilities": [],
    "public": true
  }
]
```

### Field Reference

| Field | Type | Description | Example |
|-------|------|-------------|---------|
| `process_name` | string | Must match your metadata.json package name | `"skeleton-app"` |
| `process_wasm_path` | string | Path to compiled WASM file | `"/skeleton-app.wasm"` |
| `on_exit` | string | What happens if process crashes | `"Restart"` or `"None"` |
| `request_networking` | bool | Enables P2P messaging via Request API | `true` or `false` |
| `request_capabilities` | array | System features needed | See capabilities section |
| `grant_capabilities` | array | Capabilities given to others | Usually `[]` |
| `public` | bool | Can other processes message this one | `true` or `false` |

## Where Does manifest.json Come From?

**IMPORTANT**: The `manifest.json` is generated automatically when you build with:
```bash
kit b --hyperapp
```

### Build Process Flow
```
metadata.json → kit b --hyperapp → pkg/manifest.json
     ↓                                    ↓
Package name ←────── must match ─────→ process_name
```

The build process:
1. Reads `metadata.json` for package name and publisher
2. Compiles your Rust code to WASM
3. Builds the UI with Vite
4. **Generates `pkg/manifest.json`** using metadata + defaults
5. Creates the complete package in `pkg/` directory

### If manifest.json is Missing

If you see the "failed to open file" error:
1. Check if `pkg/` directory exists
2. Run `kit b --hyperapp` to build properly
3. Verify `metadata.json` exists and is valid:
   ```json
   {
     "package": "your-app-name",
     "publisher": "your-publisher"
   }
   ```

## Customizing manifest.json

While it's auto-generated, you may need to modify it for:
- Additional capabilities
- Different crash behavior
- Security settings

### Example: Real-time Voice/WebSocket App
```json
[
  {
    "process_name": "voice-chat",
    "process_wasm_path": "/voice-chat.wasm",
    "on_exit": "Restart",
    "request_networking": true,
    "request_capabilities": [
      "homepage:homepage:sys",
      "http-server:distro:sys",  // Includes WebSocket support!
      "vfs:distro:sys"           // For storing settings/history
    ],
    "grant_capabilities": [],
    "public": false  // More secure for voice apps
  }
]
```

**Note**: WebSocket support comes with `http-server:distro:sys` - no separate capability needed! WebRTC, audio capture, and other browser APIs don't require special Hyperware capabilities.

### Example: Adding Ethereum Support
```json
[
  {
    "process_name": "my-defi-app",
    "process_wasm_path": "/my-defi-app.wasm",
    "on_exit": "Restart",
    "request_networking": true,
    "request_capabilities": [
      "homepage:homepage:sys",
      "http-client:distro:sys",
      "http-server:distro:sys",
      "vfs:distro:sys",
      "eth:distro:sys"  // Added for Ethereum
    ],
    "grant_capabilities": [],
    "public": true
  }
]
```

### Example: Granting Terminal Access
```json
[
  {
    "process_name": "system-monitor",
    "process_wasm_path": "/system-monitor.wasm",
    "on_exit": "Restart",
    "request_networking": true,
    "request_capabilities": [
      "homepage:homepage:sys",
      "http-server:distro:sys"
    ],
    "grant_capabilities": [
      {
        "process": "terminal:terminal:sys",
        "capabilities": ["messaging"]
      }
    ],
    "public": true
  }
]
```

### Example: P2P Chat Application
```json
[
  {
    "process_name": "p2p-chat",
    "process_wasm_path": "/p2p-chat.wasm",
    "on_exit": "Restart",
    "request_networking": true,
    "request_capabilities": [
      "homepage:homepage:sys",
      "http-client:distro:sys",    // Essential for P2P - make requests to other nodes
      "http-server:distro:sys",     // Serve UI and receive messages
      "vfs:distro:sys"              // Store chat history and files
    ],
    "grant_capabilities": [],       // Usually empty for apps
    "public": false                 // Privacy-focused: only known nodes can message directly
  }
]
```

**P2P Chat Requirements:**
- **http-client:distro:sys** - CRITICAL for P2P apps to send messages to other nodes
- **vfs:distro:sys** - Store message history, user preferences, and file attachments
- **public: false** - Consider for privacy-focused chat apps (HTTP endpoints still work!)

## Process Identity

Your process is identified by three parts:
```
process-name:package-name:publisher-node
```

For example:
- `skeleton-app:skeleton-app:skeleton.os`
- `samchat:samchat:samchat.os`

This identity is used:
- In P2P communication
- For capability grants
- In system logs

## on_exit Behavior

### "Restart"
- Process automatically restarts on crash
- State is preserved (if using SaveOptions)
- Good for production apps

### "None"
- Process stays dead after crash
- Requires manual restart
- Good for development/testing

## Security Considerations

### public: true
- Any process can send messages to your app
- Required for P2P features where unknown nodes connect
- Default for most apps
- Use when building open collaborative apps

### public: false
- Only processes with explicit permission can message you
- More secure but limits functionality
- Use for:
  - Voice/video chat apps (privacy)
  - Financial applications
  - System utilities
  - Apps handling sensitive data
- Note: HTTP/WebSocket endpoints still work! Only affects process-to-process messaging

## Grant Capabilities Pattern

Some applications grant capabilities to other processes. This is less common but can be useful:

```json
"grant_capabilities": [
  "homepage:homepage:sys",
  "http-client:distro:sys", 
  "terminal:terminal:sys"
]
```

**When to grant capabilities:**
- System utilities that manage other processes
- Development tools that need broad access
- Apps that act as capability proxies

**Security Note**: Be very selective about granting capabilities. Most apps should have an empty `grant_capabilities` array.

## Common Issues

### ❌ "Failed to install: process_name mismatch"
Your manifest.json process_name doesn't match metadata.json:
```json
// metadata.json
{
  "package": "my-app",  // This must match...
  ...
}

// manifest.json
{
  "process_name": "my-app",  // ...this
  ...
}
```

### ❌ "Capability not granted"
You're using a system feature without requesting it:
```rust
// Using VFS in code...
create_file("/data/file.txt")?;

// But manifest.json missing:
"request_capabilities": [
  "vfs:distro:sys"  // Add this!
]
```

### ❌ App installs but doesn't appear on homepage
Missing homepage capability:
```json
"request_capabilities": [
  "homepage:homepage:sys"  // Required for add_to_homepage()
]
```

## WebSocket Configuration

While WebSocket support is included with `http-server:distro:sys`, you need to declare WebSocket endpoints in your hyperprocess macro:

```rust
#[hyperprocess(
    endpoints = vec![
        Binding::Http { 
            path: "/api", 
            config: HttpBindingConfig::new(false, false, false, None) 
        },
        Binding::WebSocket {  // Add this for WebSocket support
            path: "/ws",
            config: HttpBindingConfig::new(false, false, false, None)
        }
    ],
)]
```

This is configured in your Rust code, not manifest.json!

## Build & Deploy Flow

1. **Write your code** with capabilities in mind
2. **Build**: `kit b --hyperapp`
3. **Check**: Verify `pkg/manifest.json` was created
4. **Modify** if needed (usually not necessary)
5. **Install**: `kit s` to start and install
6. **Test** with multiple nodes for P2P features

## Skeleton App Configuration

The skeleton app includes a minimal `manifest.json` with only essential capabilities:
- **homepage:homepage:sys** - To appear on the Hyperware homepage
- **http-server:distro:sys** - To serve UI and handle API requests

When extending the skeleton, you'll likely need to add capabilities like:
- **vfs:distro:sys** - For file storage
- **sqlite:distro:sys** - For database (also requires vfs!)
- **http-client:distro:sys** - For external API calls
- See the [Capabilities Guide](./09-CAPABILITIES-GUIDE.md) for the complete list

## Next Steps

Now that you understand manifest.json, check out:
- [Capabilities Guide](./09-CAPABILITIES-GUIDE.md) for detailed capability reference
- [P2P Patterns](./04-P2P-PATTERNS.md) for multi-node deployment
- [Testing Guide](./06-TESTING-DEBUGGING.md) for debugging deployment issues