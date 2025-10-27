# 📚 Complete Examples Reference

This guide provides complete, working examples of different types of Hyperware applications. Each example is self-contained and demonstrates specific patterns.

## Example 1: Todo List with P2P Sync

A collaborative todo list where items sync between nodes.

### Backend (lib.rs)
```rust
use hyperprocess_macro::*;
use hyperware_process_lib::{our, Address, ProcessId, Request, homepage::add_to_homepage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct TodoItem {
    pub id: String,
    pub text: String,
    pub completed: bool,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Default, Serialize, Deserialize)]
pub struct TodoState {
    todos: HashMap<String, TodoItem>,
    shared_with: Vec<String>, // Nodes we're sharing with
    sync_enabled: bool,
}

#[hyperprocess(
    name = "P2P Todo",
    ui = Some(HttpBindingConfig::default()),
    endpoints = vec![
        Binding::Http { 
            path: "/api", 
            config: HttpBindingConfig::new(false, false, false, None) 
        }
    ],
    save_config = SaveOptions::EveryMessage,
    wit_world = "todo-app-dot-os-v0"
)]
impl TodoState {
    #[init]
    async fn initialize(&mut self) {
        add_to_homepage("P2P Todo", Some("📝"), Some("/"), None);
        self.sync_enabled = true;
        println!("P2P Todo initialized on {}", our().node);
    }
    
    // Create a new todo
    #[http]
    async fn create_todo(&mut self, request_body: String) -> Result<String, String> {
        let text: String = serde_json::from_str(&request_body)
            .map_err(|_| "Invalid todo text".to_string())?;
        
        let todo = TodoItem {
            id: uuid::Uuid::new_v4().to_string(),
            text,
            completed: false,
            created_by: our().node.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };
        
        let id = todo.id.clone();
        self.todos.insert(id.clone(), todo.clone());
        
        // Sync with peers
        if self.sync_enabled {
            self.broadcast_todo_update(todo).await;
        }
        
        Ok(serde_json::json!({ "id": id }).to_string())
    }
    
    // Get all todos
    #[http]
    async fn get_todos(&self, _request_body: String) -> String {
        let todos: Vec<&TodoItem> = self.todos.values().collect();
        serde_json::to_string(&todos).unwrap()
    }
    
    // Toggle todo completion
    #[http]
    async fn toggle_todo(&mut self, request_body: String) -> Result<String, String> {
        let id: String = serde_json::from_str(&request_body)
            .map_err(|_| "Invalid ID".to_string())?;
        
        if let Some(todo) = self.todos.get_mut(&id) {
            todo.completed = !todo.completed;
            todo.updated_at = chrono::Utc::now().to_rfc3339();
            
            if self.sync_enabled {
                self.broadcast_todo_update(todo.clone()).await;
            }
            
            Ok("Toggled".to_string())
        } else {
            Err("Todo not found".to_string())
        }
    }
    
    // Share with another node
    #[http]
    async fn share_with_node(&mut self, request_body: String) -> Result<String, String> {
        let node: String = serde_json::from_str(&request_body)?;
        
        if !self.shared_with.contains(&node) {
            self.shared_with.push(node.clone());
        }
        
        // Send initial sync
        self.sync_todos_with_node(node).await?;
        
        Ok("Shared successfully".to_string())
    }
    
    // Handle incoming todo updates
    #[remote]
    async fn receive_todo_update(&mut self, todo_json: String) -> Result<String, String> {
        let todo: TodoItem = serde_json::from_str(&todo_json)?;
        
        // Update or insert based on timestamp
        match self.todos.get(&todo.id) {
            Some(existing) if existing.updated_at < todo.updated_at => {
                self.todos.insert(todo.id.clone(), todo);
            }
            None => {
                self.todos.insert(todo.id.clone(), todo);
            }
            _ => {} // Our version is newer
        }
        
        Ok("ACK".to_string())
    }
    
    // Handle sync request
    #[remote]
    async fn handle_sync_request(&self, _request: String) -> Result<String, String> {
        let todos: Vec<&TodoItem> = self.todos.values().collect();
        Ok(serde_json::to_string(&todos).unwrap())
    }
    
    // Helper methods (in same impl block for hyperprocess)
    async fn broadcast_todo_update(&self, todo: TodoItem) {
        let wrapper = json!({
            "ReceiveTodoUpdate": serde_json::to_string(&todo).unwrap()
        });
        
        let process_id = "todo-app:todo-app:skeleton.os".parse::<ProcessId>().unwrap();
        
        for node in &self.shared_with {
            let target = Address::new(node.clone(), process_id.clone());
            let _ = Request::new()
                .target(target)
                .body(serde_json::to_vec(&wrapper).unwrap())
                .expects_response(5)
                .send();
        }
    }
    
    async fn sync_todos_with_node(&self, node: String) -> Result<(), String> {
        let process_id = "todo-app:todo-app:skeleton.os".parse::<ProcessId>()
            .map_err(|e| format!("Invalid process ID: {}", e))?;
        
        let target = Address::new(node, process_id);
        let wrapper = json!({ "HandleSyncRequest": "" });
        
        let response = Request::new()
            .target(target)
            .body(serde_json::to_vec(&wrapper).unwrap())
            .expects_response(10)
            .send_and_await_response(10)
            .map_err(|e| format!("Sync failed: {:?}", e))?;
        
        if let Ok(body) = response.body() {
            let remote_todos: Vec<TodoItem> = serde_json::from_slice(&body)?;
            // Merge logic would go here
            println!("Received {} todos from peer", remote_todos.len());
        }
        
        Ok(())
    }
}
```

### Frontend (App.tsx)
```typescript
import React, { useState, useEffect } from 'react';
import { create } from 'zustand';

// Types
interface TodoItem {
  id: string;
  text: string;
  completed: boolean;
  created_by: string;
  created_at: string;
  updated_at: string;
}

// API
const api = {
  async createTodo(text: string): Promise<{ id: string }> {
    const res = await fetch('/api', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ CreateTodo: text }),
    });
    return res.json();
  },

  async getTodos(): Promise<TodoItem[]> {
    const res = await fetch('/api', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ GetTodos: "" }),
    });
    return res.json();
  },

  async toggleTodo(id: string): Promise<void> {
    await fetch('/api', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ ToggleTodo: id }),
    });
  },

  async shareWithNode(node: string): Promise<void> {
    await fetch('/api', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ ShareWithNode: node }),
    });
  },
};

// Store
interface TodoStore {
  todos: TodoItem[];
  isLoading: boolean;
  fetchTodos: () => Promise<void>;
  createTodo: (text: string) => Promise<void>;
  toggleTodo: (id: string) => Promise<void>;
  shareWith: (node: string) => Promise<void>;
}

const useTodoStore = create<TodoStore>((set, get) => ({
  todos: [],
  isLoading: false,

  fetchTodos: async () => {
    set({ isLoading: true });
    try {
      const todos = await api.getTodos();
      set({ todos, isLoading: false });
    } catch (error) {
      console.error('Failed to fetch todos:', error);
      set({ isLoading: false });
    }
  },

  createTodo: async (text: string) => {
    await api.createTodo(text);
    await get().fetchTodos();
  },

  toggleTodo: async (id: string) => {
    await api.toggleTodo(id);
    await get().fetchTodos();
  },

  shareWith: async (node: string) => {
    await api.shareWithNode(node);
  },
}));

// Components
export function TodoApp() {
  const { todos, isLoading, fetchTodos, createTodo, toggleTodo, shareWith } = useTodoStore();
  const [newTodo, setNewTodo] = useState('');
  const [shareNode, setShareNode] = useState('');

  useEffect(() => {
    fetchTodos();
    // Poll for updates
    const interval = setInterval(fetchTodos, 5000);
    return () => clearInterval(interval);
  }, []);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (newTodo.trim()) {
      await createTodo(newTodo.trim());
      setNewTodo('');
    }
  };

  const handleShare = async () => {
    if (shareNode.trim()) {
      try {
        await shareWith(shareNode);
        alert(`Shared with ${shareNode}`);
        setShareNode('');
      } catch (error) {
        alert(`Failed to share: ${error}`);
      }
    }
  };

  return (
    <div className="todo-app">
      <h1>P2P Todo List</h1>
      
      <div className="share-section">
        <input
          type="text"
          placeholder="Node to share with (e.g., bob.os)"
          value={shareNode}
          onChange={(e) => setShareNode(e.target.value)}
        />
        <button onClick={handleShare}>Share</button>
      </div>

      <form onSubmit={handleSubmit}>
        <input
          type="text"
          placeholder="Add a new todo..."
          value={newTodo}
          onChange={(e) => setNewTodo(e.target.value)}
        />
        <button type="submit">Add</button>
      </form>

      {isLoading && todos.length === 0 ? (
        <p>Loading...</p>
      ) : (
        <ul className="todo-list">
          {todos.map((todo) => (
            <li key={todo.id} className={todo.completed ? 'completed' : ''}>
              <input
                type="checkbox"
                checked={todo.completed}
                onChange={() => toggleTodo(todo.id)}
              />
              <span>{todo.text}</span>
              <small>by {todo.created_by}</small>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
```

---

## Example 2: Real-time Collaborative Notepad

A notepad where multiple users can edit simultaneously.

### Backend
```rust
use hyperprocess_macro::*;
use hyperware_process_lib::{our, Address, ProcessId, Request, homepage::add_to_homepage};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct DocumentOp {
    pub id: String,
    pub op_type: OpType,
    pub position: usize,
    pub content: String,
    pub author: String,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum OpType {
    Insert,
    Delete,
}

#[derive(Default, Serialize, Deserialize)]
pub struct NotepadState {
    document: String,
    operations: Vec<DocumentOp>,
    collaborators: Vec<String>,
    operation_counter: u64,
}

#[hyperprocess(
    name = "Collaborative Notepad",
    ui = Some(HttpBindingConfig::default()),
    endpoints = vec![
        Binding::Http { 
            path: "/api", 
            config: HttpBindingConfig::new(false, false, false, None) 
        }
    ],
    save_config = SaveOptions::OnInterval(10),
    wit_world = "notepad-dot-os-v0"
)]
impl NotepadState {
    #[init]
    async fn initialize(&mut self) {
        add_to_homepage("Notepad", Some("📄"), Some("/"), None);
        self.document = String::new();
        println!("Collaborative Notepad initialized");
    }
    
    // Get current document
    #[http]
    async fn get_document(&self, _request_body: String) -> String {
        serde_json::json!({
            "content": self.document,
            "collaborators": self.collaborators,
            "version": self.operation_counter,
        }).to_string()
    }
    
    // Apply local edit
    #[http]
    async fn edit_document(&mut self, request_body: String) -> Result<String, String> {
        #[derive(Deserialize)]
        struct EditRequest {
            op_type: String,
            position: usize,
            content: String,
        }
        
        let req: EditRequest = serde_json::from_str(&request_body)?;
        
        let op = DocumentOp {
            id: format!("{}-{}", our().node, self.operation_counter),
            op_type: match req.op_type.as_str() {
                "insert" => OpType::Insert,
                "delete" => OpType::Delete,
                _ => return Err("Invalid operation type".to_string()),
            },
            position: req.position,
            content: req.content,
            author: our().node.clone(),
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
        };
        
        // Apply operation locally
        self.apply_operation(&op)?;
        
        // Broadcast to collaborators
        self.broadcast_operation(op.clone()).await;
        
        Ok(serde_json::json!({ "version": self.operation_counter }).to_string())
    }
    
    // Join collaboration
    #[http]
    async fn join_collaboration(&mut self, request_body: String) -> Result<String, String> {
        let node: String = serde_json::from_str(&request_body)?;
        
        // Request current state from node
        let process_id = "notepad:notepad:skeleton.os".parse::<ProcessId>()?;
        let target = Address::new(node.clone(), process_id);
        
        let wrapper = json!({ "RequestState": our().node });
        
        let response = Request::new()
            .target(target)
            .body(serde_json::to_vec(&wrapper).unwrap())
            .expects_response(10)
            .send_and_await_response(10)?;
        
        if let Ok(body) = response.body() {
            let state: serde_json::Value = serde_json::from_slice(&body)?;
            self.document = state["document"].as_str().unwrap_or("").to_string();
            self.operation_counter = state["version"].as_u64().unwrap_or(0);
        }
        
        if !self.collaborators.contains(&node) {
            self.collaborators.push(node);
        }
        
        Ok("Joined".to_string())
    }
    
    // Handle incoming operations
    #[remote]
    async fn receive_operation(&mut self, op_json: String) -> Result<String, String> {
        let op: DocumentOp = serde_json::from_str(&op_json)?;
        
        // Check if we've already applied this operation
        if !self.operations.iter().any(|o| o.id == op.id) {
            self.apply_operation(&op)?;
            
            // Forward to other collaborators (gossip)
            self.broadcast_operation(op).await;
        }
        
        Ok("ACK".to_string())
    }
    
    // Handle state request
    #[remote]
    async fn request_state(&mut self, requester: String) -> Result<String, String> {
        if !self.collaborators.contains(&requester) {
            self.collaborators.push(requester);
        }
        
        Ok(serde_json::json!({
            "document": self.document,
            "version": self.operation_counter,
        }).to_string())
    }
    
    // Apply operation to document
    fn apply_operation(&mut self, op: &DocumentOp) -> Result<(), String> {
        match op.op_type {
            OpType::Insert => {
                if op.position <= self.document.len() {
                    self.document.insert_str(op.position, &op.content);
                } else {
                    return Err("Invalid position".to_string());
                }
            }
            OpType::Delete => {
                let end = (op.position + op.content.len()).min(self.document.len());
                self.document.replace_range(op.position..end, "");
            }
        }
        
        self.operations.push(op.clone());
        self.operation_counter += 1;
        
        // Limit operation history
        if self.operations.len() > 1000 {
            self.operations.drain(0..100);
        }
        
        Ok(())
    }
    
    // Broadcast operation to all collaborators
    async fn broadcast_operation(&self, op: DocumentOp) {
        let wrapper = json!({
            "ReceiveOperation": serde_json::to_string(&op).unwrap()
        });
        
        let process_id = "notepad:notepad:skeleton.os".parse::<ProcessId>().unwrap();
        
        for node in &self.collaborators {
            if node != &op.author { // Don't send back to author
                let target = Address::new(node.clone(), process_id.clone());
                let _ = Request::new()
                    .target(target)
                    .body(serde_json::to_vec(&wrapper).unwrap())
                    .expects_response(5)
                    .send();
            }
        }
    }
}
```

---

## Example 3: Distributed Key-Value Store

A simple distributed database with eventual consistency.

### Backend
```rust
use hyperprocess_macro::*;
use hyperware_process_lib::{our, Address, ProcessId, Request, homepage::add_to_homepage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone)]
pub struct KVEntry {
    pub key: String,
    pub value: String,
    pub version: u64,
    pub updated_by: String,
    pub updated_at: String,
}

#[derive(Serialize, Deserialize)]
pub struct ReplicationLog {
    pub entries: Vec<KVEntry>,
    pub from_node: String,
    pub timestamp: String,
}

#[derive(Default, Serialize, Deserialize)]
pub struct KVStore {
    data: HashMap<String, KVEntry>,
    replicas: Vec<String>,
    replication_enabled: bool,
    last_sync: HashMap<String, String>, // node -> timestamp
}

#[hyperprocess(
    name = "Distributed KV Store",
    ui = Some(HttpBindingConfig::default()),
    endpoints = vec![
        Binding::Http { 
            path: "/api", 
            config: HttpBindingConfig::new(false, false, false, None) 
        }
    ],
    save_config = SaveOptions::EveryMessage,
    wit_world = "kvstore-dot-os-v0"
)]
impl KVStore {
    #[init]
    async fn initialize(&mut self) {
        add_to_homepage("KV Store", Some("🗄️"), Some("/"), None);
        self.replication_enabled = true;
        
        // Start periodic sync
        self.schedule_periodic_sync();
    }
    
    // Set a key-value pair
    #[http]
    async fn set(&mut self, request_body: String) -> Result<String, String> {
        #[derive(Deserialize)]
        struct SetRequest {
            key: String,
            value: String,
        }
        
        let req: SetRequest = serde_json::from_str(&request_body)?;
        
        let entry = KVEntry {
            key: req.key.clone(),
            value: req.value,
            version: self.data.get(&req.key).map(|e| e.version + 1).unwrap_or(1),
            updated_by: our().node.clone(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };
        
        self.data.insert(req.key.clone(), entry.clone());
        
        // Replicate to other nodes
        if self.replication_enabled {
            self.replicate_entry(entry).await;
        }
        
        Ok("OK".to_string())
    }
    
    // Get a value by key
    #[http]
    async fn get(&self, request_body: String) -> Result<String, String> {
        let key: String = serde_json::from_str(&request_body)?;
        
        match self.data.get(&key) {
            Some(entry) => Ok(serde_json::json!({
                "value": entry.value,
                "version": entry.version,
                "updated_by": entry.updated_by,
                "updated_at": entry.updated_at,
            }).to_string()),
            None => Err("Key not found".to_string()),
        }
    }
    
    // List all keys
    #[http]
    async fn list_keys(&self, _request_body: String) -> String {
        let keys: Vec<String> = self.data.keys().cloned().collect();
        serde_json::to_string(&keys).unwrap()
    }
    
    // Add a replica node
    #[http]
    async fn add_replica(&mut self, request_body: String) -> Result<String, String> {
        let node: String = serde_json::from_str(&request_body)?;
        
        if !self.replicas.contains(&node) {
            self.replicas.push(node.clone());
            
            // Send initial sync
            self.sync_with_replica(node).await?;
        }
        
        Ok("Replica added".to_string())
    }
    
    // Handle incoming replication
    #[remote]
    async fn replicate(&mut self, entry_json: String) -> Result<String, String> {
        let entry: KVEntry = serde_json::from_str(&entry_json)?;
        
        // Apply if newer
        match self.data.get(&entry.key) {
            Some(existing) if existing.version < entry.version => {
                self.data.insert(entry.key.clone(), entry);
            }
            None => {
                self.data.insert(entry.key.clone(), entry);
            }
            _ => {} // Our version is newer
        }
        
        Ok("ACK".to_string())
    }
    
    // Handle sync request
    #[remote]
    async fn sync(&mut self, since_json: String) -> Result<String, String> {
        let since: String = serde_json::from_str(&since_json)?;
        
        let entries: Vec<KVEntry> = self.data.values()
            .filter(|e| e.updated_at > since)
            .cloned()
            .collect();
        
        let log = ReplicationLog {
            entries,
            from_node: our().node.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        
        Ok(serde_json::to_string(&log).unwrap())
    }
    
    // Replicate a single entry
    async fn replicate_entry(&self, entry: KVEntry) {
        let wrapper = json!({
            "Replicate": serde_json::to_string(&entry).unwrap()
        });
        
        let process_id = "kvstore:kvstore:skeleton.os".parse::<ProcessId>().unwrap();
        
        for replica in &self.replicas {
            let target = Address::new(replica.clone(), process_id.clone());
            let _ = Request::new()
                .target(target)
                .body(serde_json::to_vec(&wrapper).unwrap())
                .expects_response(5)
                .send();
        }
    }
    
    // Sync with a specific replica
    async fn sync_with_replica(&mut self, node: String) -> Result<(), String> {
        let last_sync = self.last_sync.get(&node)
            .cloned()
            .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string());
        
        let process_id = "kvstore:kvstore:skeleton.os".parse::<ProcessId>()?;
        let target = Address::new(node.clone(), process_id);
        
        let wrapper = json!({ "Sync": last_sync });
        
        let response = Request::new()
            .target(target)
            .body(serde_json::to_vec(&wrapper).unwrap())
            .expects_response(10)
            .send_and_await_response(10)?;
        
        if let Ok(body) = response.body() {
            let log: ReplicationLog = serde_json::from_slice(&body)?;
            
            for entry in log.entries {
                match self.data.get(&entry.key) {
                    Some(existing) if existing.version < entry.version => {
                        self.data.insert(entry.key.clone(), entry);
                    }
                    None => {
                        self.data.insert(entry.key.clone(), entry);
                    }
                    _ => {}
                }
            }
            
            self.last_sync.insert(node, log.timestamp);
        }
        
        Ok(())
    }
    
    // Schedule periodic sync
    fn schedule_periodic_sync(&self) {
        // In real implementation, use timer API
        // timer::set_timer(60000, Some(json!({ "action": "sync" })));
    }
}
```

---

## Example 4: P2P File Sharing

Share files directly between nodes.

### Backend Snippet
```rust
#[derive(Serialize, Deserialize)]
pub struct SharedFile {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub mime_type: String,
    pub owner: String,
    pub shared_with: Vec<String>,
    pub uploaded_at: String,
}

#[http]
async fn upload_file(&mut self, name: String, mime_type: String, data: Vec<u8>) -> Result<String, String> {
    let file_id = uuid::Uuid::new_v4().to_string();
    let file_path = format!("/fileshare:{}/files/{}", our().node, file_id);
    
    // Store in VFS
    let vfs_address = Address::new(our().node.clone(), "vfs:distro:sys".parse::<ProcessId>().unwrap());
    
    let write_request = json!({
        "path": file_path,
        "action": "Write"
    });
    
    Request::new()
        .target(vfs_address)
        .body(serde_json::to_vec(&write_request).unwrap())
        .blob(LazyLoadBlob::new(Some("file"), data.clone()))
        .expects_response(5)
        .send_and_await_response(5)?;
    
    let file = SharedFile {
        id: file_id.clone(),
        name,
        size: data.len() as u64,
        mime_type,
        owner: our().node.clone(),
        shared_with: vec![],
        uploaded_at: chrono::Utc::now().to_rfc3339(),
    };
    
    self.files.insert(file_id.clone(), file);
    Ok(file_id)
}

#[http]
async fn share_file(&mut self, request_body: String) -> Result<String, String> {
    #[derive(Deserialize)]
    struct ShareRequest {
        file_id: String,
        node: String,
    }
    
    let req: ShareRequest = serde_json::from_str(&request_body)?;
    
    if let Some(file) = self.files.get_mut(&req.file_id) {
        if !file.shared_with.contains(&req.node) {
            file.shared_with.push(req.node.clone());
        }
        
        // Notify the node
        let process_id = "fileshare:fileshare:skeleton.os".parse::<ProcessId>()?;
        let target = Address::new(req.node, process_id);
        
        let notification = json!({
            "FileSharedWithYou": serde_json::to_string(&file).unwrap()
        });
        
        Request::new()
            .target(target)
            .body(serde_json::to_vec(&notification).unwrap())
            .expects_response(5)
            .send()?;
        
        Ok("Shared".to_string())
    } else {
        Err("File not found".to_string())
    }
}

#[remote]
async fn request_file(&self, file_id: String) -> Result<Vec<u8>, String> {
    // Check if requester has access
    let requester = our().source.node.clone();
    
    if let Some(file) = self.files.get(&file_id) {
        if file.owner != our().node && !file.shared_with.contains(&requester) {
            return Err("Access denied".to_string());
        }
        
        // Read from VFS
        let file_path = format!("/fileshare:{}/files/{}", our().node, file_id);
        let vfs_address = Address::new(our().node.clone(), "vfs:distro:sys".parse::<ProcessId>().unwrap());
        
        let read_request = json!({
            "path": file_path,
            "action": "Read"
        });
        
        let response = Request::new()
            .target(vfs_address)
            .body(serde_json::to_vec(&read_request).unwrap())
            .expects_response(5)
            .send_and_await_response(5)?;
        
        if let Some(blob) = response.blob() {
            Ok(blob.bytes)
        } else {
            Err("File data not found".to_string())
        }
    } else {
        Err("File not found".to_string())
    }
}
```

---

## Tips for Building Your Own Apps

### 1. Start with the Skeleton
```bash
cp -r hyperapp-skeleton myapp
cd myapp
# Update metadata.json with your app name
# Modify skeleton-app to match your app name
```

### 2. Common Modifications

#### Change App Name
1. Update `metadata.json`
2. Update `Cargo.toml` (both workspace and app)
3. Rename `skeleton-app` directory
4. Update imports and ProcessId strings

#### Add Dependencies
```toml
# In your-app/Cargo.toml
[dependencies]
# ... existing deps
uuid = { version = "1.4.1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
```

#### Add Complex State
```rust
// Use JSON for complex internal state
#[derive(Default, Serialize, Deserialize)]
pub struct AppState {
    // Simple types for WIT
    pub item_count: u32,
    pub last_update: String,
    
    // Complex types as JSON
    #[serde(skip)]
    complex_data: HashMap<String, ComplexType>,
}

impl AppState {
    fn save_complex_data(&mut self) {
        // Serialize to JSON when needed
        self.complex_json = serde_json::to_string(&self.complex_data).unwrap();
    }
}
```

### 3. Testing Your App

#### Local Testing
```bash
# Build
kit b --hyperapp

# Run
kit s

# Check logs
# Backend logs appear in terminal
# Frontend logs in browser console
```

#### P2P Testing
```bash
# Terminal 1
kit s --fake-node alice.os

# Terminal 2
kit s --fake-node bob.os --port 8081

# Test communication between nodes
```

### 4. Common Gotchas

1. **Always** include `_request_body` in HTTP handlers
2. **Always** send complex parameters as JSON objects (not tuples)
3. **Always** set timeout on remote requests
4. **Never** forget the `/our.js` script
5. **Test** P2P features with multiple nodes early

---

## Example 5: Real-time Voice Chat Room

A voice chat application with WebSocket support, role-based access, and real-time state management.

### Backend (lib.rs)
```rust
use hyperprocess_macro::*;
use hyperware_process_lib::{
    our, Message, Address, ProcessId, Request, Response, 
    LazyLoadBlob, bindings::get_server, send_and_await_response
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallInfo {
    pub id: String,
    pub host_id: String,
    pub created_at: String,
    pub participant_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantInfo {
    pub id: String,
    pub node_id: String,
    pub display_name: Option<String>,
    pub role: Role,
    pub is_muted: bool,
    pub is_speaking: bool,
    pub joined_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Role {
    Admin,
    Speaker,
    Listener,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub sender_id: String,
    pub sender_name: String,
    pub content: String,
    pub timestamp: String,
}

// WebSocket message types
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WSMessage {
    // Client -> Server
    JoinCall { call_id: String, auth_token: Option<String> },
    LeaveCall,
    Chat { content: String },
    Mute { is_muted: bool },
    UpdateRole { target_id: String, new_role: Role },
    AudioData { data: String, sequence: u32 },
    Heartbeat,
    
    // Server -> Client
    JoinSuccess { 
        participant_id: String, 
        role: Role,
        participants: Vec<ParticipantInfo>,
        chat_history: Vec<ChatMessage>,
    },
    JoinError { reason: String },
    ParticipantJoined { participant: ParticipantInfo },
    ParticipantLeft { participant_id: String },
    ChatMessage { message: ChatMessage },
    RoleUpdated { participant_id: String, new_role: Role },
    ParticipantMuted { participant_id: String, is_muted: bool },
    SpeakingStateUpdated { participant_id: String, is_speaking: bool },
    AudioStream { participant_id: String, data: String, sequence: u32 },
    CallEnded { reason: String },
}

#[derive(Default, Serialize, Deserialize)]
pub struct VoiceChatState {
    calls: HashMap<String, CallInfo>,
    participants: HashMap<String, HashMap<String, ParticipantInfo>>, // call_id -> participant_id -> info
    chat_history: HashMap<String, Vec<ChatMessage>>, // call_id -> messages
    node_auth: HashMap<String, String>, // auth_token -> node_id
    ws_connections: HashMap<String, String>, // ws_channel_id -> participant_id
}

#[hyperprocess(
    name = "Voice Chat",
    ui = Some(HttpBindingConfig::default()),
    endpoints = vec![
        Binding::Http { 
            path: "/api", 
            config: HttpBindingConfig::new(false, false, false, None) 
        },
        Binding::WebSocket {
            path: "/ws",
            config: HttpBindingConfig::new(false, false, false, None)
        }
    ],
    save_config = SaveOptions::EveryMessage,
    wit_world = "voice-chat-dot-os-v0"
)]
impl VoiceChatState {
    #[init]
    async fn initialize(&mut self) {
        println!("Voice Chat initialized on {}", our().node);
    }
    
    // HTTP endpoint to create a new call
    #[http]
    async fn create_call(&mut self, _request_body: String) -> Result<String, String> {
        let call = CallInfo {
            id: uuid::Uuid::new_v4().to_string(),
            host_id: our().node.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            participant_count: 0,
        };
        
        let call_id = call.id.clone();
        self.calls.insert(call_id.clone(), call);
        self.participants.insert(call_id.clone(), HashMap::new());
        self.chat_history.insert(call_id.clone(), Vec::new());
        
        // Serve dynamic UI for this call
        get_server().serve_ui(&our(), format!("/call/{}", call_id), vec![], None, true)?;
        
        Ok(serde_json::json!({ "call_id": call_id }).to_string())
    }
    
    // Node authentication endpoint (for cross-node access)
    #[local]
    #[remote]
    async fn node_handshake(&mut self, call_id: String) -> Result<String, String> {
        if !self.calls.contains_key(&call_id) {
            return Err("Call not found".to_string());
        }
        
        let auth_token = uuid::Uuid::new_v4().to_string();
        self.node_auth.insert(auth_token.clone(), source().node);
        
        Ok(auth_token)
    }
    
    // WebSocket handler
    #[ws]
    fn handle_ws(&mut self, channel_id: u32, message_type: WsMessageType, payload: LazyLoadBlob) {
        match message_type {
            WsMessageType::Open => {
                println!("WebSocket opened: {}", channel_id);
            }
            WsMessageType::Close => {
                self.handle_ws_close(channel_id);
            }
            WsMessageType::Message => {
                if let Ok(text) = String::from_utf8(payload.bytes) {
                    if let Ok(msg) = serde_json::from_str::<WSMessage>(&text) {
                        self.handle_ws_message(channel_id, msg);
                    }
                }
            }
            WsMessageType::Error => {
                println!("WebSocket error: {}", channel_id);
            }
        }
    }
    
    fn handle_ws_message(&mut self, channel_id: u32, msg: WSMessage) {
        match msg {
            WSMessage::JoinCall { call_id, auth_token } => {
                self.handle_join_call(channel_id, call_id, auth_token);
            }
            WSMessage::LeaveCall => {
                self.handle_leave_call(channel_id);
            }
            WSMessage::Chat { content } => {
                self.handle_chat_message(channel_id, content);
            }
            WSMessage::Mute { is_muted } => {
                self.handle_mute(channel_id, is_muted);
            }
            WSMessage::UpdateRole { target_id, new_role } => {
                self.handle_role_update(channel_id, target_id, new_role);
            }
            WSMessage::AudioData { data, sequence } => {
                self.handle_audio_data(channel_id, data, sequence);
            }
            WSMessage::Heartbeat => {
                // Just acknowledge heartbeat by doing nothing
            }
            _ => {}
        }
    }
    
    fn handle_join_call(&mut self, channel_id: u32, call_id: String, auth_token: Option<String>) {
        let ws_channel_id = channel_id.to_string();
        
        // Verify call exists
        if !self.calls.contains_key(&call_id) {
            self.send_ws_message(channel_id, WSMessage::JoinError { 
                reason: "Call not found".to_string() 
            });
            return;
        }
        
        // Determine participant info
        let (node_id, role) = if let Some(token) = auth_token {
            // Authenticated node connection
            if let Some(node) = self.node_auth.get(&token) {
                (node.clone(), Role::Speaker)
            } else {
                self.send_ws_message(channel_id, WSMessage::JoinError { 
                    reason: "Invalid auth token".to_string() 
                });
                return;
            }
        } else {
            // Anonymous browser connection
            (format!("guest-{}", channel_id), Role::Listener)
        };
        
        // Create participant
        let participant_id = uuid::Uuid::new_v4().to_string();
        let participant = ParticipantInfo {
            id: participant_id.clone(),
            node_id,
            display_name: None,
            role: role.clone(),
            is_muted: true, // Everyone starts muted
            is_speaking: false,
            joined_at: chrono::Utc::now().to_rfc3339(),
        };
        
        // Add to state
        self.participants.get_mut(&call_id).unwrap()
            .insert(participant_id.clone(), participant.clone());
        self.ws_connections.insert(ws_channel_id, participant_id.clone());
        
        // Update participant count
        if let Some(call) = self.calls.get_mut(&call_id) {
            call.participant_count += 1;
        }
        
        // Send success response
        let participants: Vec<ParticipantInfo> = self.participants.get(&call_id)
            .unwrap().values().cloned().collect();
        let chat_history = self.chat_history.get(&call_id)
            .unwrap().clone();
        
        self.send_ws_message(channel_id, WSMessage::JoinSuccess {
            participant_id: participant_id.clone(),
            role,
            participants,
            chat_history,
        });
        
        // Notify others
        self.broadcast_to_call(&call_id, WSMessage::ParticipantJoined { 
            participant: participant.clone() 
        }, Some(&participant_id));
    }
    
    fn handle_audio_data(&mut self, channel_id: u32, data: String, sequence: u32) {
        if let Some(participant_id) = self.ws_connections.get(&channel_id.to_string()) {
            if let Some((call_id, _)) = self.find_participant_call(participant_id) {
                if let Some(participant) = self.participants.get(&call_id)
                    .and_then(|p| p.get(participant_id)) {
                    
                    // Only speakers/admins can send audio
                    if participant.role == Role::Speaker || participant.role == Role::Admin {
                        if !participant.is_muted {
                            // Create mix-minus audio for each listener
                            self.broadcast_audio_to_call(&call_id, participant_id, data, sequence);
                        }
                    }
                }
            }
        }
    }
    
    fn broadcast_audio_to_call(&self, call_id: &str, sender_id: &str, data: String, sequence: u32) {
        if let Some(participants) = self.participants.get(call_id) {
            for (pid, _) in participants {
                if pid != sender_id { // Mix-minus: don't send audio back to sender
                    if let Some(channel) = self.find_channel_for_participant(pid) {
                        self.send_ws_message(channel, WSMessage::AudioStream {
                            participant_id: sender_id.to_string(),
                            data: data.clone(),
                            sequence,
                        });
                    }
                }
            }
        }
    }
    
    // Helper methods
    fn find_participant_call(&self, participant_id: &str) -> Option<(String, ParticipantInfo)> {
        for (call_id, participants) in &self.participants {
            if let Some(info) = participants.get(participant_id) {
                return Some((call_id.clone(), info.clone()));
            }
        }
        None
    }
    
    fn find_channel_for_participant(&self, participant_id: &str) -> Option<u32> {
        self.ws_connections.iter()
            .find(|(_, pid)| *pid == participant_id)
            .and_then(|(channel, _)| channel.parse::<u32>().ok())
    }
    
    fn send_ws_message(&self, channel_id: u32, msg: WSMessage) {
        if let Ok(json) = serde_json::to_string(&msg) {
            get_server().send_ws_push(
                channel_id,
                WsMessageType::Text,
                LazyLoadBlob::new(Some("message"), json.into_bytes())
            );
        }
    }
    
    fn broadcast_to_call(&self, call_id: &str, msg: WSMessage, exclude: Option<&str>) {
        if let Some(participants) = self.participants.get(call_id) {
            for (pid, _) in participants {
                if exclude.map_or(true, |ex| ex != pid) {
                    if let Some(channel) = self.find_channel_for_participant(pid) {
                        self.send_ws_message(channel, msg.clone());
                    }
                }
            }
        }
    }
}
```

### Frontend Store (voiceStore.ts)
```typescript
import { create } from 'zustand';
import { AudioServiceV3 } from './services/audio-service';

interface VoiceState {
  // Connection
  ws: WebSocket | null;
  connectionStatus: 'disconnected' | 'connecting' | 'connected';
  
  // Call info
  callId: string | null;
  myId: string | null;
  myRole: 'Admin' | 'Speaker' | 'Listener' | null;
  participants: Map<string, ParticipantInfo>;
  chatMessages: ChatMessage[];
  
  // Audio
  audioService: AudioServiceV3 | null;
  isMuted: boolean;
  speakingStates: Map<string, boolean>;
  
  // Actions
  joinCall: (callId: string, authToken?: string) => void;
  leaveCall: () => void;
  sendMessage: (content: string) => void;
  toggleMute: () => void;
  updateRole: (targetId: string, newRole: string) => void;
}

export const useVoiceStore = create<VoiceState>((set, get) => ({
  // Initial state
  ws: null,
  connectionStatus: 'disconnected',
  callId: null,
  myId: null,
  myRole: null,
  participants: new Map(),
  chatMessages: [],
  audioService: null,
  isMuted: true,
  speakingStates: new Map(),
  
  // Actions
  joinCall: (callId: string, authToken?: string) => {
    const ws = new WebSocket(`${window.location.protocol.replace('http', 'ws')}//${window.location.host}/ws`);
    
    ws.onopen = () => {
      set({ ws, connectionStatus: 'connected' });
      ws.send(JSON.stringify({ 
        type: 'JoinCall', 
        call_id: callId,
        auth_token: authToken 
      }));
    };
    
    ws.onmessage = async (event) => {
      const msg = JSON.parse(event.data);
      
      switch (msg.type) {
        case 'JoinSuccess':
          const audioService = new AudioServiceV3(() => get());
          await audioService.initializeAudio(msg.role, msg.participant_id, false);
          
          const participantsMap = new Map();
          msg.participants.forEach(p => participantsMap.set(p.id, p));
          
          set({
            myId: msg.participant_id,
            myRole: msg.role,
            participants: participantsMap,
            chatMessages: msg.chat_history || [],
            audioService,
          });
          break;
          
        case 'AudioStream':
          const audio = get().audioService;
          if (audio) {
            await audio.handleIncomingAudio(msg.participant_id, {
              data: msg.data,
              sequence: msg.sequence,
            });
          }
          break;
          
        case 'SpeakingStateUpdated':
          set(state => ({
            speakingStates: new Map(state.speakingStates).set(
              msg.participant_id, 
              msg.is_speaking
            )
          }));
          break;
          
        // Handle other message types...
      }
    };
    
    ws.onclose = () => {
      set({ ws: null, connectionStatus: 'disconnected' });
      get().audioService?.cleanup();
    };
    
    set({ ws, connectionStatus: 'connecting' });
  },
  
  toggleMute: async () => {
    const { audioService, isMuted, ws } = get();
    if (!audioService || !ws) return;
    
    const newMutedState = !isMuted;
    set({ isMuted: newMutedState });
    
    await audioService.toggleMute(newMutedState);
    ws.send(JSON.stringify({ 
      type: 'Mute', 
      is_muted: newMutedState 
    }));
  },
  
  // Other actions...
}));
```

This example demonstrates:
- WebSocket real-time communication
- Role-based access control
- Audio streaming with mix-minus
- Dynamic UI serving for call routes
- Node authentication for P2P access
- State management with Zustand
- Participant presence tracking

## Example 6: P2P Chat (Simplified from samchat)

A decentralized chat application with direct messages, group chats, and file sharing.

### Backend (lib.rs)
```rust
use hyperprocess_macro::*;
use hyperware_process_lib::{our, Address, ProcessId, Request, homepage::add_to_homepage, LazyLoadBlob};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::Utc;

// Core message type
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    id: String,
    conversation_id: String,
    sender: String,
    recipient: Option<String>,      // None for groups
    recipients: Option<Vec<String>>, // Some for groups
    content: String,
    timestamp: String,              // RFC3339 for sorting
    delivered: bool,
    reply_to: Option<MessageReplyInfo>,
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct MessageReplyInfo {
    message_id: String,
    sender: String,
    content: String,
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct Conversation {
    id: String,
    participants: Vec<String>,
    messages: Vec<ChatMessage>,
    last_updated: String,
    is_group: bool,
    group_name: Option<String>,
    created_by: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
pub struct ChatState {
    conversations: HashMap<String, Conversation>,
    my_node_id: Option<String>,
}

#[hyperprocess(
    name = "P2P Chat",
    ui = Some(HttpBindingConfig::default()),
    endpoints = vec![
        Binding::Http { 
            path: "/api", 
            config: HttpBindingConfig::new(false, false, false, None) 
        }
    ],
    save_config = SaveOptions::EveryMessage,
    wit_world = "p2p-chat-dot-os-v0"
)]
impl ChatState {
    #[init]
    async fn initialize(&mut self) {
        self.my_node_id = Some(our().node.clone());
        add_to_homepage("P2P Chat", Some("💬"), Some("/"), None);
        println!("P2P Chat initialized for node: {}", our().node);
    }
    
    // Send a message (with optional reply)
    #[http]
    async fn send_message(&mut self, request_body: String) -> Result<String, String> {
        #[derive(Deserialize)]
        struct SendRequest {
            recipient: String,
            content: String,
            reply_to: Option<MessageReplyInfo>,
        }
        
        let req: SendRequest = serde_json::from_str(&request_body)?;
        let sender = self.my_node_id.clone()
            .ok_or_else(|| "Node not initialized".to_string())?;
        
        // Determine if group or direct message
        let is_group = req.recipient.starts_with("group_");
        let conversation_id = if is_group {
            req.recipient.clone()
        } else {
            let mut participants = vec![sender.clone(), req.recipient.clone()];
            participants.sort();
            participants.join("|")
        };
        
        // Create message
        let message = ChatMessage {
            id: Uuid::new_v4().to_string(),
            conversation_id: conversation_id.clone(),
            sender: sender.clone(),
            recipient: if is_group { None } else { Some(req.recipient.clone()) },
            recipients: if is_group {
                self.conversations.get(&conversation_id)
                    .map(|c| c.participants.iter()
                        .filter(|p| *p != &sender)
                        .cloned()
                        .collect())
            } else { None },
            content: req.content,
            timestamp: Utc::now().to_rfc3339(),
            delivered: false,
            reply_to: req.reply_to,
        };
        
        // Store locally
        let conversation = self.conversations.entry(conversation_id.clone())
            .or_insert_with(|| Conversation {
                id: conversation_id.clone(),
                participants: if is_group { vec![] } else { 
                    vec![sender.clone(), req.recipient.clone()] 
                },
                messages: Vec::new(),
                last_updated: message.timestamp.clone(),
                is_group,
                group_name: None,
                created_by: None,
            });
        conversation.messages.push(message.clone());
        conversation.last_updated = message.timestamp.clone();
        
        // Send to recipient(s)
        let recipients = if is_group {
            message.recipients.clone().unwrap_or_default()
        } else {
            vec![req.recipient]
        };
        
        for recipient in recipients {
            let _ = self.send_to_node(recipient, message.clone()).await;
        }
        
        Ok(json!({ "message_id": message.id }).to_string())
    }
    
    // Create a group
    #[http]
    async fn create_group(&mut self, request_body: String) -> Result<String, String> {
        #[derive(Deserialize)]
        struct CreateGroupRequest {
            name: String,
            members: Vec<String>,
        }
        
        let req: CreateGroupRequest = serde_json::from_str(&request_body)?;
        let creator = self.my_node_id.clone()
            .ok_or_else(|| "Node not initialized".to_string())?;
        
        let mut participants = req.members;
        if !participants.contains(&creator) {
            participants.push(creator.clone());
        }
        
        let group_id = format!("group_{}", Uuid::new_v4());
        
        // Create group locally
        let conversation = Conversation {
            id: group_id.clone(),
            participants: participants.clone(),
            messages: Vec::new(),
            last_updated: Utc::now().to_rfc3339(),
            is_group: true,
            group_name: Some(req.name.clone()),
            created_by: Some(creator.clone()),
        };
        self.conversations.insert(group_id.clone(), conversation);
        
        // Notify members
        for participant in &participants {
            if participant != &creator {
                let _ = self.notify_group_join(
                    participant.clone(), 
                    group_id.clone(), 
                    req.name.clone(),
                    participants.clone()
                ).await;
            }
        }
        
        Ok(json!({ "group_id": group_id }).to_string())
    }
    
    // Get conversations
    #[http]
    async fn get_conversations(&self, _request_body: String) -> String {
        let summaries: Vec<_> = self.conversations.values()
            .map(|c| json!({
                "id": c.id,
                "participants": c.participants,
                "last_updated": c.last_updated,
                "is_group": c.is_group,
                "group_name": c.group_name,
                "last_message": c.messages.last().map(|m| &m.content),
            }))
            .collect();
        
        serde_json::to_string(&summaries).unwrap()
    }
    
    // Get messages for a conversation
    #[http]
    async fn get_messages(&self, request_body: String) -> Result<String, String> {
        let conversation_id: String = serde_json::from_str(&request_body)?;
        
        self.conversations.get(&conversation_id)
            .map(|c| serde_json::to_string(&c.messages).unwrap())
            .ok_or_else(|| "Conversation not found".to_string())
    }
    
    // Receive message from another node
    #[remote]
    async fn receive_message(&mut self, message: ChatMessage) -> Result<String, String> {
        let conversation_id = message.conversation_id.clone();
        
        let conversation = self.conversations.entry(conversation_id.clone())
            .or_insert_with(|| {
                if message.recipients.is_some() {
                    // Group message
                    Conversation {
                        id: conversation_id.clone(),
                        participants: vec![message.sender.clone()],
                        messages: Vec::new(),
                        last_updated: message.timestamp.clone(),
                        is_group: true,
                        group_name: None,
                        created_by: None,
                    }
                } else {
                    // Direct message
                    let mut participants = vec![
                        message.sender.clone(), 
                        message.recipient.clone().unwrap_or_default()
                    ];
                    participants.sort();
                    Conversation {
                        id: conversation_id.clone(),
                        participants,
                        messages: Vec::new(),
                        last_updated: message.timestamp.clone(),
                        is_group: false,
                        group_name: None,
                        created_by: None,
                    }
                }
            });
        
        // Avoid duplicates
        if !conversation.messages.iter().any(|m| m.id == message.id) {
            conversation.messages.push(message.clone());
            conversation.messages.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
            conversation.last_updated = Utc::now().to_rfc3339();
        }
        
        Ok("ACK".to_string())
    }
    
    // Handle group join notification
    #[remote]
    async fn handle_group_join(&mut self, group_id: String, name: String, participants: Vec<String>) -> Result<String, String> {
        let conversation = Conversation {
            id: group_id.clone(),
            participants,
            messages: Vec::new(),
            last_updated: Utc::now().to_rfc3339(),
            is_group: true,
            group_name: Some(name),
            created_by: None,
        };
        
        self.conversations.insert(group_id, conversation);
        Ok("Joined".to_string())
    }
    
    // Helper methods
    async fn send_to_node(&self, node: String, message: ChatMessage) -> Result<(), String> {
        let publisher = "skeleton.os"; // Must match across all nodes!
        let process_id = format!("p2p-chat:p2p-chat:{}", publisher)
            .parse::<ProcessId>()
            .map_err(|e| format!("Invalid ProcessId: {}", e))?;
        
        let target = Address::new(node, process_id);
        let wrapper = json!({ "ReceiveMessage": message });
        
        Request::new()
            .target(target)
            .body(serde_json::to_vec(&wrapper).unwrap())
            .expects_response(30)
            .send()
            .map(|_| ())
            .map_err(|e| format!("Send failed: {:?}", e))
    }
    
    async fn notify_group_join(&self, node: String, group_id: String, name: String, participants: Vec<String>) -> Result<(), String> {
        let publisher = "skeleton.os";
        let process_id = format!("p2p-chat:p2p-chat:{}", publisher)
            .parse::<ProcessId>()?;
        
        let target = Address::new(node, process_id);
        let wrapper = json!({ 
            "HandleGroupJoin": [group_id, name, participants]
        });
        
        Request::new()
            .target(target)
            .body(serde_json::to_vec(&wrapper).unwrap())
            .expects_response(30)
            .send()
            .map(|_| ())
            .map_err(|e| format!("Notification failed: {:?}", e))
    }
}
```

### Frontend Component (ChatApp.tsx)
```typescript
import React, { useState, useEffect, useRef } from 'react';
import { create } from 'zustand';

// Store
const useChatStore = create((set, get) => ({
  conversations: [],
  currentConversation: null,
  messages: [],
  myNodeId: window.our?.node || 'unknown',
  
  fetchConversations: async () => {
    const res = await fetch('/api', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ GetConversations: "" })
    });
    const data = await res.json();
    set({ conversations: data });
  },
  
  selectConversation: async (convId) => {
    set({ currentConversation: convId });
    const res = await fetch('/api', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ GetMessages: convId })
    });
    const messages = await res.json();
    set({ messages });
  },
  
  sendMessage: async (recipient, content, replyTo = null) => {
    await fetch('/api', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ 
        SendMessage: { recipient, content, reply_to: replyTo }
      })
    });
    await get().fetchConversations();
    if (get().currentConversation) {
      await get().selectConversation(get().currentConversation);
    }
  },
  
  createGroup: async (name, members) => {
    const res = await fetch('/api', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ CreateGroup: { name, members } })
    });
    const { group_id } = await res.json();
    await get().fetchConversations();
    return group_id;
  }
}));

// Main component
export function ChatApp() {
  const { 
    conversations, 
    currentConversation, 
    messages, 
    myNodeId,
    fetchConversations,
    selectConversation,
    sendMessage,
    createGroup
  } = useChatStore();
  
  const [messageText, setMessageText] = useState('');
  const [newChatAddress, setNewChatAddress] = useState('');
  const [replyingTo, setReplyingTo] = useState(null);
  const messageListRef = useRef(null);
  
  useEffect(() => {
    fetchConversations();
    const interval = setInterval(fetchConversations, 3000); // Poll for updates
    return () => clearInterval(interval);
  }, []);
  
  useEffect(() => {
    // Auto-scroll to bottom
    if (messageListRef.current) {
      messageListRef.current.scrollTop = messageListRef.current.scrollHeight;
    }
  }, [messages]);
  
  const handleSend = async () => {
    if (!messageText.trim()) return;
    
    const recipient = currentConversation || newChatAddress;
    if (!recipient) return;
    
    await sendMessage(recipient, messageText, replyingTo);
    setMessageText('');
    setReplyingTo(null);
    setNewChatAddress('');
  };
  
  const formatTime = (timestamp) => {
    const date = new Date(timestamp);
    const today = new Date().toDateString() === date.toDateString();
    return today 
      ? date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
      : date.toLocaleDateString();
  };
  
  return (
    <div className="chat-app">
      <div className="sidebar">
        <h2>P2P Chat</h2>
        <div className="node-info">Node: {myNodeId}</div>
        
        <button onClick={() => {
          const name = prompt('Group name:');
          const members = prompt('Members (comma-separated):')?.split(',').map(m => m.trim());
          if (name && members) createGroup(name, members);
        }}>
          + New Group
        </button>
        
        <div className="conversation-list">
          {conversations.map(conv => (
            <div 
              key={conv.id}
              className={`conversation ${currentConversation === conv.id ? 'active' : ''}`}
              onClick={() => selectConversation(conv.id)}
            >
              <div className="name">
                {conv.is_group ? `👥 ${conv.group_name}` : conv.participants.find(p => p !== myNodeId)}
              </div>
              <div className="last-message">{conv.last_message}</div>
              <div className="time">{formatTime(conv.last_updated)}</div>
            </div>
          ))}
        </div>
      </div>
      
      <div className="chat-main">
        {currentConversation ? (
          <>
            <div className="message-list" ref={messageListRef}>
              {messages.map(msg => (
                <div 
                  key={msg.id} 
                  className={`message ${msg.sender === myNodeId ? 'sent' : 'received'}`}
                >
                  {msg.reply_to && (
                    <div className="reply-context">
                      ↩️ {msg.reply_to.sender}: {msg.reply_to.content}
                    </div>
                  )}
                  <div className="sender">{msg.sender}</div>
                  <div className="content">{msg.content}</div>
                  <div className="time">{formatTime(msg.timestamp)}</div>
                  <button 
                    className="reply-btn"
                    onClick={() => setReplyingTo({
                      message_id: msg.id,
                      sender: msg.sender,
                      content: msg.content
                    })}
                  >
                    Reply
                  </button>
                </div>
              ))}
            </div>
            
            <div className="message-input">
              {replyingTo && (
                <div className="replying-to">
                  Replying to {replyingTo.sender}
                  <button onClick={() => setReplyingTo(null)}>✕</button>
                </div>
              )}
              <input
                type="text"
                value={messageText}
                onChange={(e) => setMessageText(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleSend()}
                placeholder="Type a message..."
              />
              <button onClick={handleSend}>Send</button>
            </div>
          </>
        ) : (
          <div className="no-conversation">
            <h3>Start a new conversation</h3>
            <input
              type="text"
              value={newChatAddress}
              onChange={(e) => setNewChatAddress(e.target.value)}
              placeholder="Enter node address (e.g., alice.os)"
            />
            {newChatAddress && (
              <div className="new-chat-input">
                <input
                  type="text"
                  value={messageText}
                  onChange={(e) => setMessageText(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && handleSend()}
                  placeholder="Type your first message..."
                />
                <button onClick={handleSend}>Send</button>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
```

This simplified P2P chat example demonstrates:
- Direct messaging between nodes
- Group chat creation and member notifications
- Message replies with context
- Conversation management
- Real-time updates via polling
- Clean separation of concerns
- No centralized server required

Key differences from samchat:
- Simplified type structure
- No file attachments (can be added)
- Basic UI without all features
- Clear code organization for learning

## Remember

These examples show patterns, not prescriptions. Adapt them to your needs:
- Simplify for single-node apps
- Add complexity for advanced features
- Mix patterns as needed
- Keep security in mind
- Design for offline-first
- Test edge cases
- Consider WebSockets for real-time features
- Use dynamic UI serving for user-specific content
- Start simple, then add features incrementally