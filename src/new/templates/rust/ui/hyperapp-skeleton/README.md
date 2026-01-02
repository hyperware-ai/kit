# Hyperware Skeleton App

A minimal, well-commented skeleton application for the Hyperware platform using the Hyperapp framework.
This skeleton provides a starting point for building Hyperware applications with a React/TypeScript frontend and Rust backend.

Example prompt (works well with Codex):

```
Use `kit new myappname --template hyperapp-skeleton --ui`, (replacing myappname with appropriate app name) to make a template in `/desired_folder`, which you will modify to build the following app:

Insert your app spec here, e.g.:
Todo List with P2P Sync.
A collaborative TODO list where items sync between nodes.

Write a spec, and then implement it step by step.
Use the README.md given in hyperapp-skeleton to find instructions on specific details.
At the end, I should be able to run `kit build --hyperapp && kit s` and manually test that the app works.
```

The rest of this document is aimed at *LLMs* not *humans*.

## Quick Start

### Building

Always build with
```bash
kit build --hyperapp
```

## Project Structure

```
hyperapp-skeleton/
├── Cargo.toml          # Workspace configuration
├── metadata.json       # App metadata
├── hyperapp-skeleton/       # Main Rust process
│   ├── Cargo.toml      # Process dependencies
│   └── src/
│       ├── lib.rs      # Main app logic (well-commented)
│       └── icon        # App icon file
├── ui/                 # Frontend application
│   ├── package.json    # Node dependencies
│   ├── index.html      # Entry point (includes /our.js)
│   ├── vite.config.ts  # Build configuration
│   └── src/
│       ├── App.tsx     # Main React component
│       ├── store/      # Zustand state management
│       ├── types/      # TypeScript type definitions
│       └── utils/      # API utilities
├── api/                # Generated WIT files (after build)
└── pkg/                # The final build product, including manifest.json, scripts.json and built package output
```

## Key Concepts

### 1. The Hyperprocess Macro

The `#[hyperapp_macro::hyperapp]` macro is the core of the Hyperapp framework.
It provides:
- Async/await support without tokio
- Automatic WIT generation
- State persistence
- HTTP/WebSocket endpoint configuration

### 2. Required Patterns

#### HTTP Endpoints
ALL HTTP endpoints MUST be tagged with `#[http]`:
```rust
#[http]
async fn my_endpoint(&self) -> String {
    // Implementation
}
```

#### Remote Requests
All remote requests must set `.expects_response(time)`, where `time` is the response timeout, in seconds.
```rust
let req = Request::to(("friend.os", "some-hyperapp", "some-hyperapp", "publisher.os"))
    .expects_response(30)
    .blob(LazyLoadBlob {
        mime: None,
        bytes: message,
    })
    .body(body);
```

#### Frontend API Calls
Parameters must be sent as tuples for multi-parameter methods:
```typescript
// Single parameter
{ "MethodName": value }

// Multiple parameters
{ "MethodName": [param1, param2] }
```

#### Frontend keys in snake_case
All keys in TypeScript need to stay in snake_case (e.g. `node_id`).
camelCase (e.g. `nodeId`) will break the app!
```typescript
export interface StatusSnapshot {
    node_id: string;
  }
```

#### The /our.js Script
MUST be included in index.html:
```html
<script src="/our.js"></script>
```

### 3. State Persistence

Your app's state is automatically persisted based on the `save_config` option:
- `OnDiff`: Save when state changes (strongly recommended)
- `Never`: No automatic saves
- `EveryMessage`: Save after each message (safest; slowest)
- `EveryNMessage(u64)`: Save every N messages received
- `EveryNSeconds(u64)`: Save every N seconds

## Customization Guide

### 1. Modify App State

Edit `AppState` in `hyperapp-skeleton/src/lib.rs`:
```rust
#[derive(Default, Serialize, Deserialize)]
pub struct AppState {
    // Add your fields here
    my_data: Vec<MyType>,
}
```
Rename it to `*State` where `*` is the name of the app, e.g., `foo` -> `FooState`.

### 2. Add HTTP Endpoints

For UI interaction:
```rust
#[http]
async fn my_method(&mut self) -> Result<String, String> {
    // Parse request, update state, return response
}
```

### 3. Add Capabilities

Add system permissions in `pkg/manifest.json`:
```json
"request_capabilities": [
    "homepage:homepage:sys",
    "http-server:distro:sys",
    "vfs:distro:sys"
]
```

These are required to message other local processes.
They can also be granted so other local processes can message us.

If sending messages between nodes, set:
```json
"request_networking": true,
```

### 4. Update Frontend

1. Add types in `ui/src/types/hyperapp-skeleton.ts`
2. Update store in `ui/src/store/hyperapp-skeleton.ts`
3. Modify UI in `ui/src/App.tsx`

## Common Issues and Solutions

### "Failed to deserialize HTTP request"
- Check parameter format (tuple vs object)

### "Node not connected"
- Verify `/our.js` is included in index.html
- Check that the app is running in Hyperware environment

### WIT Generation Errors
- No fixed arrays (use Vec<T>)
- Add #[derive(PartialEq)] to structs

### Naming Restrictions
- No struct/enum/interface name is allowed to contain digits or the substring "stream", because WIT doesn't allow it
- No record/variant/enum name is allowed to end with `Request`, `Response`, `RequestWrapper`, `ResponseWrapper`, because TS caller utils are autogenerated with those suffixes

## Instructions

Carefully read the prompt; look carefully at `instructions.md` (if it exists) and in the example-apps directory.

In `example-apps/`:
- `sign` and `id` demonstrate local messaging.
- `file-explorer` demonstrates VFS interactions.
  The `file-explorer` example contains an `api` folder, which is generated by the compiler, and not human or LLM written.

Look at the `hyperware_process_lib` and in particular at `hyperapp`.
If not provided with local repo look at https://github.com/hyperware-ai/process_lib/tree/main/src and https://raw.githubusercontent.com/hyperware-ai/process_lib/refs/heads/main/src/hyperapp.rs

Work from the existing template, whose backend lives at `my-app-name/` and frontend at `ui/`.

Bindings for the UI will be generated when the app is built with `kit build --hyperapp`.
Thus, first design and implement the backend; the interface will be generated from the backend; finally design and implement the frontend to consume the interface.
Subsequent changes to the interface must follow this pattern as well: start in backend, generate interface, finish in frontend

Do NOT create the API.
The API is machine generated.
You create types that end up in the API by defining and using them in functions in the Rust backend "hyperapp"

When sending p2p always send using `hyperware_process_lib::hyperapp::send` or `hyperware_process_lib::hyperapp::send_rmp` when expecting a Response, never `.send_and_await_response`.
Example usage of `send` and `send_rmp` are shown in example_apps.
If not expecting a response, use `hyperware_process_lib::Request::send`.

`send` and `send_rmp` handle (de)serialization.
Just put Rust types as args and return Rust types as return values.
Do not return String or JSON if a more specific Rust type will work.

If you create a GUI for the app you MUST use target/ui/caller-utils.ts for HTTP requests to the backend.
Do NOT edit this file: it is machine generated.
Do NOT do `fetch` or other HTTP requests manually to the backend: use the functions in this machine generated interface.
