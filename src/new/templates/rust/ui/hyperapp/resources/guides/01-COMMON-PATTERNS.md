# 📋 Common Patterns & Copy-Paste Recipes

## Table of Contents
1. [State Management Patterns](#state-management-patterns)
2. [HTTP Endpoint Patterns](#http-endpoint-patterns)
3. [P2P Communication Patterns](#p2p-communication-patterns)
4. [File Handling Patterns](#file-handling-patterns)
5. [Real-time Update Patterns](#real-time-update-patterns)
6. [Error Handling Patterns](#error-handling-patterns)
7. [UI State Management](#ui-state-management)
8. [Authentication & Permissions](#authentication--permissions)
9. [Group Chat Patterns](#group-chat-patterns)
10. [Timer Patterns](#timer-patterns)

---

## State Management Patterns

### Basic App State Structure
```rust
#[derive(Default, Serialize, Deserialize)]
pub struct AppState {
    // Identity
    my_node_id: Option<String>,
    
    // Core data
    items: Vec<Item>,
    users: Vec<User>,
    
    // Indexes for fast lookup
    item_by_id: HashMap<String, usize>,
    
    // Temporary/UI state
    pending_operations: Vec<String>,
}

// Initialize in #[init]
#[init]
async fn initialize(&mut self) {
    self.my_node_id = Some(our().node.clone());
    add_to_homepage("My App", Some("🚀"), Some("/"), None);
}
```

### State with Versioning
```rust
#[derive(Default, Serialize, Deserialize)]
pub struct AppState {
    version: u32,
    data: StateData,
}

#[derive(Default, Serialize, Deserialize)]
pub struct StateData {
    // Your actual state fields
}

impl AppState {
    fn migrate(&mut self) {
        match self.version {
            0 => {
                // Migrate from v0 to v1
                self.version = 1;
            },
            1 => {
                // Already latest
            },
            _ => {}
        }
    }
}
```

---

## HTTP Endpoint Patterns

### CRUD Operations
```rust
// CREATE
#[http]
async fn create_item(&mut self, request_body: String) -> Result<String, String> {
    #[derive(Deserialize)]
    struct CreateRequest {
        name: String,
        description: String,
    }
    
    let req: CreateRequest = serde_json::from_str(&request_body)
        .map_err(|e| format!("Invalid request: {}", e))?;
    
    let item = Item {
        id: uuid::Uuid::new_v4().to_string(),
        name: req.name,
        description: req.description,
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    
    let id = item.id.clone();
    self.items.push(item);
    
    Ok(serde_json::json!({ "id": id }).to_string())
}

// READ (single)
#[http]
async fn get_item(&self, request_body: String) -> Result<String, String> {
    let id: String = serde_json::from_str(&request_body)
        .map_err(|_| "Invalid ID".to_string())?;
    
    self.items.iter()
        .find(|item| item.id == id)
        .map(|item| serde_json::to_string(item).unwrap())
        .ok_or_else(|| "Item not found".to_string())
}

// READ (list with pagination)
#[http]
async fn list_items(&self, request_body: String) -> String {
    #[derive(Deserialize)]
    struct ListRequest {
        page: usize,
        per_page: usize,
    }
    
    let req: ListRequest = serde_json::from_str(&request_body)
        .unwrap_or(ListRequest { page: 0, per_page: 20 });
    
    let start = req.page * req.per_page;
    let end = (start + req.per_page).min(self.items.len());
    
    let items: Vec<_> = self.items[start..end].to_vec();
    let total = self.items.len();
    
    serde_json::json!({
        "items": items,
        "total": total,
        "page": req.page,
        "per_page": req.per_page,
    }).to_string()
}

// UPDATE
#[http]
async fn update_item(&mut self, request_body: String) -> Result<String, String> {
    #[derive(Deserialize)]
    struct UpdateRequest {
        id: String,
        name: Option<String>,
        description: Option<String>,
    }
    
    let req: UpdateRequest = serde_json::from_str(&request_body)
        .map_err(|e| format!("Invalid request: {}", e))?;
    
    let item = self.items.iter_mut()
        .find(|item| item.id == req.id)
        .ok_or_else(|| "Item not found".to_string())?;
    
    if let Some(name) = req.name {
        item.name = name;
    }
    if let Some(description) = req.description {
        item.description = description;
    }
    
    Ok("Updated".to_string())
}

// DELETE
#[http]
async fn delete_item(&mut self, request_body: String) -> Result<String, String> {
    let id: String = serde_json::from_str(&request_body)
        .map_err(|_| "Invalid ID".to_string())?;
    
    let initial_len = self.items.len();
    self.items.retain(|item| item.id != id);
    
    if self.items.len() < initial_len {
        Ok("Deleted".to_string())
    } else {
        Err("Item not found".to_string())
    }
}
```

### Search and Filter
```rust
#[http]
async fn search_items(&self, request_body: String) -> String {
    #[derive(Deserialize)]
    struct SearchRequest {
        query: String,
        tags: Option<Vec<String>>,
        sort_by: Option<String>,
    }
    
    let req: SearchRequest = serde_json::from_str(&request_body)
        .unwrap_or(SearchRequest { 
            query: String::new(), 
            tags: None, 
            sort_by: None 
        });
    
    let mut results: Vec<_> = self.items.iter()
        .filter(|item| {
            // Text search
            let matches_query = req.query.is_empty() || 
                item.name.to_lowercase().contains(&req.query.to_lowercase()) ||
                item.description.to_lowercase().contains(&req.query.to_lowercase());
            
            // Tag filter
            let matches_tags = req.tags.as_ref().map_or(true, |tags| {
                tags.iter().any(|tag| item.tags.contains(tag))
            });
            
            matches_query && matches_tags
        })
        .cloned()
        .collect();
    
    // Sort
    match req.sort_by.as_deref() {
        Some("name") => results.sort_by(|a, b| a.name.cmp(&b.name)),
        Some("created") => results.sort_by(|a, b| a.created_at.cmp(&b.created_at)),
        _ => {}
    }
    
    serde_json::to_string(&results).unwrap()
}
```

---

## P2P Communication Patterns

### Basic Remote Call
```rust
// Remote endpoint on receiving node
#[remote]
async fn handle_sync_request(&mut self, data: String) -> Result<String, String> {
    let request: SyncRequest = serde_json::from_str(&data)?;
    
    // Process request
    let response = SyncResponse {
        items: self.items.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    
    Ok(serde_json::to_string(&response).unwrap())
}

// Making the call from another node
#[http]
async fn sync_with_node(&mut self, request_body: String) -> Result<String, String> {
    let target_node: String = serde_json::from_str(&request_body)?;
    
    // Construct address
    let process_id = format!("skeleton-app:skeleton-app:{}", "skeleton.os")
        .parse::<ProcessId>()
        .map_err(|e| format!("Invalid process ID: {}", e))?;
    
    let target_address = Address::new(target_node, process_id);
    
    // Create request
    let sync_request = SyncRequest {
        since_timestamp: self.last_sync,
    };
    
    let wrapper = json!({
        "HandleSyncRequest": serde_json::to_string(&sync_request).unwrap()
    });
    
    // Send and await
    let response = Request::new()
        .target(target_address)
        .body(serde_json::to_vec(&wrapper).unwrap())
        .expects_response(30)
        .send_and_await_response(30)
        .map_err(|e| format!("Sync failed: {:?}", e))?;
    
    // Process response
    if let Ok(body) = response.body() {
        let sync_response: SyncResponse = serde_json::from_slice(&body)?;
        self.merge_items(sync_response.items);
        self.last_sync = Some(sync_response.timestamp);
        Ok("Sync successful".to_string())
    } else {
        Err("No response body".to_string())
    }
}
```

### Broadcast Pattern
```rust
// Send notification to multiple nodes
#[http]
async fn broadcast_update(&mut self, request_body: String) -> Result<String, String> {
    let update: UpdateNotification = serde_json::from_str(&request_body)?;
    
    let mut success_count = 0;
    let mut errors = Vec::new();
    
    for node in &self.connected_nodes {
        let process_id = format!("skeleton-app:skeleton-app:{}", "skeleton.os")
            .parse::<ProcessId>()
            .unwrap();
        
        let target = Address::new(node.clone(), process_id);
        let wrapper = json!({
            "HandleUpdateNotification": serde_json::to_string(&update).unwrap()
        });
        
        // Fire and forget - don't wait for response
        match Request::new()
            .target(target)
            .body(serde_json::to_vec(&wrapper).unwrap())
            .expects_response(5)
            .send() {
                Ok(_) => success_count += 1,
                Err(e) => errors.push(format!("{}: {:?}", node, e)),
            }
    }
    
    Ok(serde_json::json!({
        "sent_to": success_count,
        "errors": errors,
    }).to_string())
}
```

### Request-Reply with Retry
```rust
async fn reliable_remote_call(
    &self,
    target_node: String,
    method: &str,
    data: String,
    max_retries: u32,
) -> Result<String, String> {
    let process_id = format!("skeleton-app:skeleton-app:{}", "skeleton.os")
        .parse::<ProcessId>()
        .map_err(|e| format!("Invalid process ID: {}", e))?;
    
    let target = Address::new(target_node.clone(), process_id);
    let wrapper = json!({ method: data });
    
    for attempt in 0..max_retries {
        if attempt > 0 {
            // Exponential backoff
            let delay = std::time::Duration::from_millis(100 * 2u64.pow(attempt));
            timer::set_timer(delay.as_millis() as u64, None);
        }
        
        match Request::new()
            .target(target.clone())
            .body(serde_json::to_vec(&wrapper).unwrap())
            .expects_response(30)
            .send_and_await_response(30) {
                Ok(response) => {
                    if let Ok(body) = response.body() {
                        return Ok(String::from_utf8_lossy(&body).to_string());
                    }
                },
                Err(e) if attempt < max_retries - 1 => {
                    println!("Retry {}/{}: {:?}", attempt + 1, max_retries, e);
                    continue;
                },
                Err(e) => return Err(format!("Failed after {} retries: {:?}", max_retries, e)),
            }
    }
    
    Err("Max retries exceeded".to_string())
}
```

---

## File Handling Patterns

**Note**: These patterns require the `"vfs:distro:sys"` capability and importing `Address` and `ProcessId` from hyperware_process_lib.

### File Upload
```rust
#[http]
async fn upload_file(&mut self, file_name: String, mime_type: String, file_data: Vec<u8>) -> Result<String, String> {
    let file_id = uuid::Uuid::new_v4().to_string();
    let file_path = format!("/skeleton-app:skeleton.os/files/{}", file_id);
    
    // Create directory if needed
    let vfs_address = Address::new(our().node.clone(), "vfs:distro:sys".parse::<ProcessId>().unwrap());
    
    let create_dir = json!({
        "path": "/skeleton-app:skeleton.os/files",
        "action": "CreateDirAll"
    });
    
    let _ = Request::new()
        .target(vfs_address.clone())
        .body(serde_json::to_vec(&create_dir).unwrap())
        .expects_response(5)
        .send_and_await_response(5);
    
    // Write file
    let write_request = json!({
        "path": file_path,
        "action": "Write"
    });
    
    Request::new()
        .target(vfs_address)
        .body(serde_json::to_vec(&write_request).unwrap())
        .blob(LazyLoadBlob::new(Some("file"), file_data.clone()))
        .expects_response(5)
        .send_and_await_response(5)
        .map_err(|e| format!("Failed to write file: {:?}", e))?;
    
    // Store metadata
    self.files.push(FileInfo {
        id: file_id.clone(),
        name: file_name,
        mime_type,
        size: file_data.len() as u64,
        uploaded_at: chrono::Utc::now().to_rfc3339(),
    });
    
    Ok(file_id)
}

// Download file
#[http]
async fn download_file(&self, request_body: String) -> Result<Vec<u8>, String> {
    let file_id: String = serde_json::from_str(&request_body)?;
    let file_path = format!("/skeleton-app:skeleton.os/files/{}", file_id);
    
    let vfs_address = Address::new(our().node.clone(), "vfs:distro:sys".parse::<ProcessId>().unwrap());
    
    let read_request = json!({
        "path": file_path,
        "action": "Read"
    });
    
    let response = Request::new()
        .target(vfs_address)
        .body(serde_json::to_vec(&read_request).unwrap())
        .expects_response(5)
        .send_and_await_response(5)
        .map_err(|e| format!("Failed to read file: {:?}", e))?;
    
    response.blob()
        .map(|blob| blob.bytes)
        .ok_or_else(|| "No file data in response".to_string())
}
```

---

## Real-time Update Patterns

### WebSocket Pattern
```rust
use hyperware_process_lib::{LazyLoadBlob, http::server::{send_ws_push, WsMessageType}};

// Configure WebSocket endpoint in hyperprocess macro
#[hyperprocess(
    endpoints = vec![
        Binding::Ws {
            path: "/ws",
            config: WsBindingConfig::default().authenticated(false),
        },
    ],
)]

// WebSocket handler
#[ws]
fn websocket(&mut self, channel_id: u32, message_type: WsMessageType, blob: LazyLoadBlob) {
    match message_type {
        WsMessageType::Text => {
            if let Ok(message) = String::from_utf8(blob.bytes) {
                // Parse and handle client message
                match serde_json::from_str::<ClientMessage>(&message) {
                    Ok(msg) => self.handle_ws_message(channel_id, msg),
                    Err(e) => self.send_ws_error(channel_id, &format!("Invalid message: {}", e)),
                }
            }
        }
        WsMessageType::Close => {
            // Clean up on disconnect
            self.handle_ws_disconnect(channel_id);
        }
        _ => {}
    }
}

// Send message to specific client
fn send_to_channel(&self, channel_id: u32, message: ServerMessage) {
    let json = serde_json::to_string(&message).unwrap();
    let blob = LazyLoadBlob {
        mime: Some("application/json".to_string()),
        bytes: json.into_bytes(),
    };
    send_ws_push(channel_id, WsMessageType::Text, blob);
}

// Broadcast to all connected clients
fn broadcast(&self, message: ServerMessage) {
    let json = serde_json::to_string(&message).unwrap();
    let bytes = json.into_bytes();
    
    for &channel_id in &self.connected_channels {
        let blob = LazyLoadBlob {
            mime: Some("application/json".to_string()),
            bytes: bytes.clone(),
        };
        send_ws_push(channel_id, WsMessageType::Text, blob);
    }
}
```

### Polling Pattern (Fallback)
```rust
// Backend: Track updates
#[derive(Default, Serialize, Deserialize)]
pub struct AppState {
    updates: Vec<Update>,
    last_update_id: u64,
}

#[http]
async fn poll_updates(&self, request_body: String) -> String {
    #[derive(Deserialize)]
    struct PollRequest {
        since_id: u64,
    }
    
    let req: PollRequest = serde_json::from_str(&request_body)
        .unwrap_or(PollRequest { since_id: 0 });
    
    let updates: Vec<_> = self.updates.iter()
        .filter(|u| u.id > req.since_id)
        .cloned()
        .collect();
    
    serde_json::json!({
        "updates": updates,
        "last_id": self.last_update_id,
    }).to_string()
}

// Frontend: Poll for updates
// store/app.ts
export const useAppStore = create<AppStore>((set, get) => ({
    // ... other state
    
    startPolling: () => {
        const pollInterval = setInterval(async () => {
            try {
                const updates = await api.pollUpdates(get().lastUpdateId);
                if (updates.updates.length > 0) {
                    set({
                        updates: [...get().updates, ...updates.updates],
                        lastUpdateId: updates.last_id,
                    });
                }
            } catch (error) {
                console.error('Polling error:', error);
            }
        }, 2000); // Poll every 2 seconds
        
        set({ pollInterval });
    },
    
    stopPolling: () => {
        const interval = get().pollInterval;
        if (interval) {
            clearInterval(interval);
            set({ pollInterval: null });
        }
    },
}));
```

---

## Dynamic UI Serving Pattern

### Serve UI at Dynamic Paths
```rust
use hyperware_app_common::{get_server, HttpBindingConfig};

// Serve UI at a dynamic path (e.g., for a chat room)
#[http]
async fn create_room(&mut self, request: CreateRoomReq) -> Result<RoomInfo, String> {
    let room_id = generate_room_id();
    
    // Create room state
    let room = Room {
        id: room_id.clone(),
        // ... other fields
    };
    self.rooms.insert(room_id.clone(), room);
    
    // Serve UI at /room/<room-id>
    let room_path = format!("/room/{}", room_id);
    get_server()
        .unwrap()
        .serve_ui(
            "ui-room", // UI bundle name
            vec![&room_path],
            HttpBindingConfig::default().authenticated(false)
        )
        .map_err(|e| format!("Failed to serve UI: {:?}", e))?;
    
    Ok(RoomInfo { id: room_id })
}

// Unserve UI when room is deleted
#[http]
async fn delete_room(&mut self, room_id: String) -> Result<(), String> {
    // Remove room
    self.rooms.remove(&room_id);
    
    // Stop serving UI
    let room_path = format!("/room/{}", room_id);
    get_server()
        .unwrap()
        .unserve_ui("ui-room", vec![&room_path])
        .map_err(|e| format!("Failed to unserve UI: {:?}", e))?;
    
    Ok(())
}
```

---

## Error Handling Patterns

### Standard Error Pattern (Use Throughout Your App)
```rust
// Convert any error to a String for HTTP responses
.map_err(|e| format!("Operation failed: {:?}", e))?;

// Example usage in endpoints
#[http]
async fn create_item(&mut self, request_body: String) -> Result<String, String> {
    let item: Item = serde_json::from_str(&request_body)
        .map_err(|e| format!("Invalid request: {}", e))?;
    
    self.save_item(item)
        .map_err(|e| format!("Failed to save item: {:?}", e))?;
    
    Ok("Created".to_string())
}
```

### Comprehensive Error Handling
```rust
// Define error types for complex apps
#[derive(Serialize, Deserialize)]
pub enum AppError {
    NotFound { resource: String },
    InvalidInput { field: String, reason: String },
    Unauthorized { action: String },
    RemoteError { node: String, error: String },
    InternalError { message: String },
}

impl AppError {
    fn to_response(&self) -> String {
        serde_json::json!({
            "error": self,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }).to_string()
    }
}

// Use in endpoints
#[http]
async fn protected_action(&mut self, request_body: String) -> Result<String, String> {
    // Parse and validate
    let req: ActionRequest = serde_json::from_str(&request_body)
        .map_err(|_| AppError::InvalidInput {
            field: "request".to_string(),
            reason: "Invalid JSON".to_string(),
        }.to_response())?;
    
    // Check permissions
    if !self.can_perform_action(&req.user_id, &req.action) {
        return Err(AppError::Unauthorized {
            action: req.action.clone(),
        }.to_response());
    }
    
    // Find resource
    let item = self.items.iter_mut()
        .find(|i| i.id == req.item_id)
        .ok_or_else(|| AppError::NotFound {
            resource: format!("item:{}", req.item_id),
        }.to_response())?;
    
    // Perform action
    match perform_action(item, req.action) {
        Ok(result) => Ok(serde_json::to_string(&result).unwrap()),
        Err(e) => Err(AppError::InternalError {
            message: e.to_string(),
        }.to_response()),
    }
}
```

---

## UI State Management

### Zustand Store with TypeScript
```typescript
// types/app.ts
export interface AppState {
    // Data
    items: Item[];
    currentUser: User | null;
    
    // UI State
    isLoading: boolean;
    error: string | null;
    selectedItemId: string | null;
    
    // Pagination
    currentPage: number;
    itemsPerPage: number;
    totalItems: number;
}

export interface AppActions {
    // Data actions
    fetchItems: (page?: number) => Promise<void>;
    createItem: (data: CreateItemData) => Promise<void>;
    updateItem: (id: string, data: UpdateItemData) => Promise<void>;
    deleteItem: (id: string) => Promise<void>;
    
    // UI actions
    selectItem: (id: string | null) => void;
    setError: (error: string | null) => void;
    clearError: () => void;
    
    // Pagination
    setPage: (page: number) => void;
}

// store/app.ts
export const useAppStore = create<AppState & AppActions>((set, get) => ({
    // Initial state
    items: [],
    currentUser: null,
    isLoading: false,
    error: null,
    selectedItemId: null,
    currentPage: 0,
    itemsPerPage: 20,
    totalItems: 0,
    
    // Actions
    fetchItems: async (page) => {
        const currentPage = page ?? get().currentPage;
        set({ isLoading: true, error: null });
        
        try {
            const response = await api.listItems({
                page: currentPage,
                per_page: get().itemsPerPage,
            });
            
            set({
                items: response.items,
                totalItems: response.total,
                currentPage,
                isLoading: false,
            });
        } catch (error) {
            set({
                error: getErrorMessage(error),
                isLoading: false,
            });
        }
    },
    
    createItem: async (data) => {
        set({ isLoading: true, error: null });
        
        try {
            const response = await api.createItem(data);
            
            // Refresh the list
            await get().fetchItems();
            
            // Select the new item
            set({ selectedItemId: response.id });
        } catch (error) {
            set({
                error: getErrorMessage(error),
                isLoading: false,
            });
            throw error; // Re-throw for form handling
        }
    },
    
    // ... other actions
}));

// Hooks for common selections
export const useSelectedItem = () => {
    const selectedId = useAppStore(state => state.selectedItemId);
    const items = useAppStore(state => state.items);
    return items.find(item => item.id === selectedId);
};

export const usePagination = () => {
    const { currentPage, itemsPerPage, totalItems, setPage } = useAppStore();
    const totalPages = Math.ceil(totalItems / itemsPerPage);
    
    return {
        currentPage,
        totalPages,
        hasNext: currentPage < totalPages - 1,
        hasPrev: currentPage > 0,
        goToPage: setPage,
        nextPage: () => setPage(currentPage + 1),
        prevPage: () => setPage(currentPage - 1),
    };
};
```

---

## Authentication & Permissions

### Role-Based Access Control
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Role {
    Listener,  // Can only listen
    Chatter,   // Can listen + chat
    Speaker,   // Can listen + chat + speak
    Admin,     // All permissions
}

// Define what each role can do
impl Role {
    fn can_chat(&self) -> bool {
        matches!(self, Role::Chatter | Role::Speaker | Role::Admin)
    }
    
    fn can_speak(&self) -> bool {
        matches!(self, Role::Speaker | Role::Admin)
    }
    
    fn can_admin(&self) -> bool {
        matches!(self, Role::Admin)
    }
}

// WebSocket with role-based permissions
fn handle_ws_message(&mut self, channel_id: u32, msg: ClientMessage) {
    let participant = match self.get_participant_by_channel(channel_id) {
        Some(p) => p,
        None => {
            self.send_ws_error(channel_id, "Not authenticated");
            return;
        }
    };
    
    match msg {
        ClientMessage::Chat(content) => {
            if !participant.role.can_chat() {
                self.send_ws_error(channel_id, "No chat permission");
                return;
            }
            // Handle chat...
        }
        ClientMessage::UpdateRole { target_id, new_role } => {
            if !participant.role.can_admin() {
                self.send_ws_error(channel_id, "No admin permission");
                return;
            }
            // Update role...
        }
        _ => {}
    }
}
```

### Node-based Identity
```rust
#[derive(Serialize, Deserialize)]
pub struct Permission {
    node: String,
    role: Role,
    granted_at: String,
}

#[derive(Serialize, Deserialize)]
pub enum Role {
    Owner,
    Admin,
    Member,
    Guest,
}

impl AppState {
    fn check_permission(&self, node: &str, required_role: Role) -> bool {
        self.permissions.iter()
            .find(|p| p.node == node)
            .map(|p| match (&p.role, &required_role) {
                (Role::Owner, _) => true,
                (Role::Admin, Role::Admin) | (Role::Admin, Role::Member) | (Role::Admin, Role::Guest) => true,
                (Role::Member, Role::Member) | (Role::Member, Role::Guest) => true,
                (Role::Guest, Role::Guest) => true,
                _ => false,
            })
            .unwrap_or(false)
    }
}

// Protected endpoint
#[http]
async fn admin_action(&mut self, request_body: String) -> Result<String, String> {
    let caller_node = our().source.node.clone();
    
    if !self.check_permission(&caller_node, Role::Admin) {
        return Err("Unauthorized: Admin access required".to_string());
    }
    
    // Perform admin action
    Ok("Admin action completed".to_string())
}
```

---

## Group Chat Patterns

### Creating and Managing Groups
```rust
#[derive(Serialize, Deserialize)]
pub struct GroupInfo {
    id: String,
    name: String,
    participants: Vec<String>,
    created_by: String,
    created_at: String,
}

// Create a group and notify members
#[http]
async fn create_group(&mut self, group_name: String, members: Vec<String>) -> Result<String, String> {
    let creator = self.my_node_id.clone()
        .ok_or_else(|| "Node ID not initialized".to_string())?;
    
    // Validate inputs
    if group_name.trim().is_empty() {
        return Err("Group name cannot be empty".to_string());
    }
    
    // Ensure creator is included
    let mut participants = members;
    if !participants.contains(&creator) {
        participants.push(creator.clone());
    }
    
    if participants.len() < 2 {
        return Err("Group must have at least 2 participants".to_string());
    }
    
    // Create group
    let group = GroupInfo {
        id: format!("group_{}", uuid::Uuid::new_v4()),
        name: group_name.clone(),
        participants: participants.clone(),
        created_by: creator.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    
    // Store locally
    self.groups.insert(group.id.clone(), group.clone());
    
    // Notify all members via P2P
    let process_id = "myapp:myapp:publisher.os".parse::<ProcessId>()?;
    
    for member in &participants {
        if member != &creator {
            let target = Address::new(member.clone(), process_id.clone());
            let wrapper = json!({
                "JoinGroup": serde_json::to_string(&group).unwrap()
            });
            
            let _ = Request::new()
                .target(target)
                .body(serde_json::to_vec(&wrapper).unwrap())
                .expects_response(5)
                .send(); // Fire and forget
        }
    }
    
    Ok(group.id)
}

// Receive group join notification
#[remote]
async fn join_group(&mut self, group_json: String) -> Result<String, String> {
    let group: GroupInfo = serde_json::from_str(&group_json)?;
    self.groups.insert(group.id.clone(), group);
    Ok("Joined".to_string())
}
```

### Message Reply Pattern
```rust
#[derive(Serialize, Deserialize)]
pub struct MessageReplyInfo {
    message_id: String,
    sender: String,
    preview: String, // First 100 chars of original message
}

#[derive(Serialize, Deserialize)]
pub struct Message {
    id: String,
    sender: String,
    content: String,
    timestamp: String,
    reply_to: Option<MessageReplyInfo>,
}

// Send message with optional reply
#[http]
async fn send_message(&mut self, recipient: String, content: String, reply_to_id: Option<String>) -> Result<String, String> {
    let reply_info = if let Some(reply_id) = reply_to_id {
        // Find the message being replied to
        self.messages.get(&reply_id)
            .map(|msg| MessageReplyInfo {
                message_id: msg.id.clone(),
                sender: msg.sender.clone(),
                preview: msg.content.chars().take(100).collect(),
            })
    } else {
        None
    };
    
    let message = Message {
        id: uuid::Uuid::new_v4().to_string(),
        sender: self.my_node_id.clone().unwrap(),
        content,
        timestamp: chrono::Utc::now().to_rfc3339(),
        reply_to: reply_info,
    };
    
    // Send to recipient...
    Ok(message.id)
}
```

---

## Timer Patterns

**⚠️ IMPORTANT**: Timer patterns require the `"timer:distro:sys"` capability in your manifest.json!

### Basic One-Time Timer

```rust
use hyperware_process_lib::timer;

// Set a timer for 5 seconds
#[http]
async fn set_reminder(&mut self, _request_body: String) -> Result<String, String> {
    // Timer fires once after 5000ms (5 seconds)
    timer::set_timer(5000, None);
    Ok("Reminder set for 5 seconds".to_string())
}

// Handle timer in your message handler (not shown in skeleton)
// In a full app, you'd handle Message::Timer in your main loop
```

### Recurring Timer Pattern

```rust
#[derive(Default, Serialize, Deserialize)]
pub struct AppState {
    sync_interval_ms: u64,
    sync_enabled: bool,
    last_sync: Option<String>,
}

// Start recurring sync
#[http]
async fn start_auto_sync(&mut self, request_body: String) -> Result<String, String> {
    let interval_ms: u64 = serde_json::from_str(&request_body)
        .unwrap_or(30000); // Default 30 seconds
    
    self.sync_interval_ms = interval_ms;
    self.sync_enabled = true;
    
    // Set initial timer
    timer::set_timer(interval_ms, Some(json!({ "action": "sync" })));
    
    Ok(format!("Auto-sync enabled every {}ms", interval_ms))
}

// In your timer handler (conceptual - depends on app structure)
async fn handle_timer(&mut self, context: Option<serde_json::Value>) {
    if let Some(ctx) = context {
        if let Some(action) = ctx.get("action").and_then(|a| a.as_str()) {
            match action {
                "sync" => {
                    // Perform sync
                    self.perform_sync().await;
                    
                    // Schedule next sync if still enabled
                    if self.sync_enabled {
                        timer::set_timer(
                            self.sync_interval_ms, 
                            Some(json!({ "action": "sync" }))
                        );
                    }
                }
                _ => {}
            }
        }
    }
}

// Stop recurring timer
#[http]
async fn stop_auto_sync(&mut self, _request_body: String) -> Result<String, String> {
    self.sync_enabled = false;
    Ok("Auto-sync disabled".to_string())
}
```

### Delayed Operation Pattern

```rust
// Schedule an operation for later
#[http]
async fn schedule_task(&mut self, request_body: String) -> Result<String, String> {
    #[derive(Deserialize)]
    struct ScheduleRequest {
        task_id: String,
        delay_seconds: u64,
        task_data: serde_json::Value,
    }
    
    let req: ScheduleRequest = serde_json::from_str(&request_body)?;
    
    // Store task info for later
    self.pending_tasks.insert(req.task_id.clone(), req.task_data.clone());
    
    // Schedule execution
    timer::set_timer(
        req.delay_seconds * 1000,
        Some(json!({
            "action": "execute_task",
            "task_id": req.task_id
        }))
    );
    
    Ok(format!("Task {} scheduled for {} seconds", req.task_id, req.delay_seconds))
}
```

### Timeout Pattern

```rust
// Operation with timeout
async fn operation_with_timeout(&mut self) -> Result<String, String> {
    // Start operation
    self.operation_in_progress = true;
    self.operation_id = uuid::Uuid::new_v4().to_string();
    
    // Set timeout
    timer::set_timer(
        10000, // 10 second timeout
        Some(json!({
            "action": "timeout",
            "operation_id": self.operation_id
        }))
    );
    
    // Start async operation...
    Ok("Operation started".to_string())
}

// In timer handler
async fn handle_timeout(&mut self, operation_id: String) {
    if self.operation_in_progress && self.operation_id == operation_id {
        // Operation timed out
        self.operation_in_progress = false;
        println!("Operation {} timed out", operation_id);
        // Clean up...
    }
}
```

### Debounce Pattern

```rust
// Debounce rapid calls
#[http]
async fn search(&mut self, request_body: String) -> Result<String, String> {
    let query: String = serde_json::from_str(&request_body)?;
    
    // Cancel previous timer if exists
    self.search_timer_active = false;
    
    // Set new timer
    self.search_timer_active = true;
    let timer_id = uuid::Uuid::new_v4().to_string();
    self.current_timer_id = timer_id.clone();
    
    timer::set_timer(
        300, // 300ms debounce
        Some(json!({
            "action": "execute_search",
            "query": query,
            "timer_id": timer_id
        }))
    );
    
    Ok("Search scheduled".to_string())
}

// Execute only if timer wasn't cancelled
async fn handle_search(&mut self, query: String, timer_id: String) {
    if self.search_timer_active && self.current_timer_id == timer_id {
        // Perform actual search
        self.perform_search(query).await;
        self.search_timer_active = false;
    }
}
```

### Important Notes on Timers

1. **Capability Required**: Add `"timer:distro:sys"` to your manifest.json
2. **No Timer Cancellation**: Once set, timers fire - use flags to ignore if needed
3. **Context Data**: Use the optional context parameter to pass data to timer handler
4. **Persistence**: Timers don't persist across app restarts
5. **Accuracy**: Timers are approximate, don't rely on millisecond precision

## Best Practices Summary

1. **Always validate input** - Use proper error types
2. **Handle errors gracefully** - Return meaningful error messages
3. **Use proper typing** - Define structs for requests/responses
4. **Implement pagination** - Don't return unbounded lists
5. **Add logging** - Use `println!` for debugging
6. **Test P2P early** - Don't wait until the end
7. **Design for offline** - Handle network failures
8. **Keep state minimal** - Only store what you need
9. **Document patterns** - Help future developers
10. **Use transactions** - Group related state changes
11. **Timer cleanup** - Track active timers to handle cancellation