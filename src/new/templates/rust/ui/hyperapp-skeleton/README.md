# Hyperware Skeleton App

A minimal, well-commented skeleton application for the Hyperware platform using the Hyperapp framework.
This skeleton provides a starting point for building Hyperware applications with a React/TypeScript frontend and Rust backend.

Either prompt your favorite LLM directly with instructions on how to build your app or add them to `instructions.md`!

Recommended usage:
- Clone this repo & clean up git-related stuff:
  ```bash
  git clone https://github.com/humanizersequel/hyperapp-skeleton.git
  cd hyperapp-skeleton
  rm -rf .git
  ```
- Write a detailed document describing what you want your app to do.
  Save this in `instructions.md`.
- Prompt your LLM agent (i.e. Claude Code) with something like:
  ```
  ## GOAL

  <One-sentence description of app here>

  ## Instructions

  Read the README.md and follow the Instructions > Create an implementation plan
  ```

- After creating an implementation plan, clear your LLM agent's context and then prompt it again with something like:

  ```
  ## GOAL

  <One-sentence description of app here>

  ## Instructions

  Read the README.md and follow the Instructions > Implement the plan
  ```

The rest of this document is aimed at *LLMs* not *humans*.

## Quick Start

### Prerequisites

- Hyperware development environment (`kit` command)
- Rust toolchain
- Node.js and npm

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
├── skeleton-app/       # Main Rust process
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

The `#[hyperprocess]` macro is the core of the Hyperapp framework. It provides:
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

#### Frontend API Calls
Parameters must be sent as tuples for multi-parameter methods:
```typescript
// Single parameter
{ "MethodName": value }

// Multiple parameters
{ "MethodName": [param1, param2] }
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
- `EveeyNMessage(u64)`: Save every N messages received
- `EveeyNSeconds(u64)`: Save every N seconds

## Customization Guide

### 1. Modify App State

Edit `AppState` in `skeleton-app/src/lib.rs`:
```rust
#[derive(Default, Serialize, Deserialize)]
pub struct AppState {
    // Add your fields here
    my_data: Vec<MyType>,
}
```

### 2. Add HTTP Endpoints

For UI interaction:
```rust
#[http]
async fn my_method(&mut self, request_body: String) -> Result<String, String> {
    // Parse request, update state, return response
}
```

### 3. Add Capabilities

Add system permissions in `pkg/manifest.json`:
```json
"request_capabilities": [
    "homepage:homepage:sys",
    "http-server:distro:sys",
    "vfs:distro:sys"  // Add as needed
]
```

These are required to message other local processes.
They can also be granted so other local processes can message us.
There is also a `request_networking` field that must be true to send messages over the network p2p.

### 4. Update Frontend

1. Add types in `ui/src/types/skeleton.ts`
2. Add API calls in `ui/src/utils/api.ts`
3. Update store in `ui/src/store/skeleton.ts`
4. Modify UI in `ui/src/App.tsx`

### 5. Rename as appropriate

Change names throughout from `skeleton-app` (and variants) as appropriate if user describes app name.

## Common Issues and Solutions

### "Failed to deserialize HTTP request"
- Check parameter format (tuple vs object)

### "Node not connected"
- Verify `/our.js` is included in index.html
- Check that the app is running in Hyperware environment

### WIT Generation Errors
- Use simple types or return JSON strings
- No HashMap (use Vec<(K,V)>)
- No fixed arrays (use Vec<T>)
- Add #[derive(PartialEq)] to structs

### Import Errors
- Don't add `hyperware_process_lib` to Cargo.toml
- Use imports from `hyperprocess_macro`

## Testing Your App

1. Deploy app to a Hyperware node (after building, if requested):
   ```bash
   kit start-packages
   ```
2. Your app will be automatically installed and available at `http://localhost:8080`
3. Check the Hyperware homepage for your app icon

## Instructions

### Create an implementation plan

Carefully read the prompt; look carefully at `instructions.md` (if it exists) and in the resources/ directory.
In particular, note the example applications `resources/example-apps/sign/`, `resources/example-apps/id/`, and `resources/example-apps/file-explorer`.
`sign` and `id` demonstrate local messaging.
`file-explorer` demonstrates VFS interactions.

Expand the prompt and/or `instructions.md` into a detailed implementation plan.
The implementor will be starting from this existing template that exists at `skeleton-app/` and `ui/`.

Note in particular that bindings for the UI will be generated when the app is built with `kit build --hyperapp`.
As such, first design and implement the backend; the interface will be generated from the backend; finally design and implement the frontend to consume the interface.
Subsequent changes to the interface must follow this pattern as well: start in backend, generate interface, finish in frontend

Do NOT create the API.
The API is machine generated.
You create types that end up in the API by defining and using them in functions in the Rust backend "hyperapp"

Do NOT write code: just create a detailed `IMPLEMENTATION_PLAN.md` that will be used by the implementor.
The implementor will have access to `resources/` but will be working from `IMPLEMENTATION_PLAN.md`, so include all relevant context in the PLAN.
You can refer the implementor to `resources/` but do not assume the implementor has read them unless you refer them there.

### Implement the plan

Look carefully at `IMPLEMENTATION_PLAN.md` and in the `resources/` directory, if relevant.
In particular, note the example applications `resources/example-apps/sign/`, `resources/example-apps/id/`, and `resources/example-apps/file-explorer`.
Use them if useful.

Work from the existing template that exists at `skeleton-app/` and `ui/`.

Note in particular that bindings for the UI will be generated when the app is built with `kit build --hyperapp`.
As such, first design and implement the backend; the interface will be generated from the backend; finally design and implement the frontend to consume the interface.
Subsequent changes to the interface must follow this pattern as well: start in backend, generate interface, finish in frontend

Do NOT create the API.
The API is machine generated.
You create types that end up in the API by defining and using them in functions in the Rust backend "hyperapp"

Do not worry about serialization/deserialization when using `send` and `send_rmp` functions for p2p communication.
Notice that this all happens within those functions: just take the rust types as args and return rust types as return values.

If you create a GUI for the app you MUST use target/ui/caller-utils.ts for HTTP requests to the backend.
Do NOT edit this file: it is machine generated.
Do NOT do `fetch` or other HTTP requests manually to the backend: use the functions in this machine generated interface.

Implement the application described in the `IMPLEMENTATION_PLAN.md`.
