# 🌐 P2P Communication Patterns Guide

## Core Concepts

In Hyperware, every user runs their own node. P2P communication allows nodes to:
- Share data directly without central servers
- Coordinate actions across the network
- Build collaborative applications
- Maintain distributed state

## Endpoint Attributes

Understanding the different endpoint types:

- **`#[http]`** - HTTP endpoints accessible via frontend API calls
- **`#[remote]`** - Endpoints callable by other nodes via P2P
- **`#[local]`** - Internal endpoints callable within the same node
- **`#[local] #[remote]`** - Endpoints callable both locally and remotely

```rust
// HTTP only - frontend calls
#[http]
async fn get_data(&self, _request_body: String) -> Vec<Item> { }

// Remote only - other nodes call this
#[remote]
async fn sync_data(&mut self, data: String) -> Result<String, String> { }

// Both local and remote - flexible access
#[local]
#[remote]
async fn process_request(&mut self, req: Request) -> Result<Response, String> { }
```

## Essential Components

### 1. Node Identity
```rust
// Get your own node identity
let my_node = our().node.clone();  // e.g., "alice.os"

// Node identity comes from the user, not hardcoded
#[http]
async fn connect_to_node(&mut self, request_body: String) -> Result<String, String> {
    let target_node: String = serde_json::from_str(&request_body)?;
    // Use target_node for communication
}
```

### 2. Process Identity
```rust
// ProcessId format: "process-name:package-name:publisher"
// Note: publisher must match between communicating nodes
let process_id = "myapp:myapp:publisher.os"
    .parse::<ProcessId>()
    .map_err(|e| format!("Invalid ProcessId: {}", e))?;

// For your app to talk to itself on other nodes
// IMPORTANT: All nodes must use the same publisher!
let my_process_id = format!("{}:{}:{}", 
    "skeleton-app",      // process name (from metadata.json)
    "skeleton-app",      // package name (from metadata.json)
    "skeleton.os"        // publisher (must be consistent across nodes)
).parse::<ProcessId>()?;
```

### 3. Address Construction
```rust
// Combine node + process to create full address
let target_address = Address::new(
    "bob.os".to_string(),     // target node
    process_id                 // target process
);
```

### 4. Request Patterns

Two ways to make P2P requests:

**Traditional Pattern (hyperware_process_lib::Request):**
```rust
let response = Request::new()
    .target(target_address)
    .body(serde_json::to_vec(&data).unwrap())
    .expects_response(30)
    .send_and_await_response(30)?;
```

**Modern Pattern (hyperware_app_common::send):**
```rust
use hyperware_app_common::send;

// Type-safe request with automatic deserialization
let request = Request::to(&target_address)
    .body(serde_json::to_vec(&data).unwrap());

match send::<Result<ResponseType, String>>(request).await {
    Ok(Ok(response)) => {
        // Use response directly - already deserialized
    }
    Ok(Err(e)) => {
        // Remote returned an error
    }
    Err(e) => {
        // Network/communication error
    }
}
```

## P2P Communication Patterns

### Pattern 1: Direct Request-Response

**Use Case:** Query data from another node

```rust
// On the requesting node
#[http]
async fn get_remote_data(&self, request_body: String) -> Result<String, String> {
    let target_node: String = serde_json::from_str(&request_body)?;
    
    // Build address
    let process_id = "skeleton-app:skeleton-app:skeleton.os".parse::<ProcessId>()?;
    let target = Address::new(target_node, process_id);
    
    // Create request
    let request_data = json!({
        "since": self.last_sync_time,
        "limit": 100
    });
    
    // Wrap for remote method
    let wrapper = json!({
        "GetDataSince": serde_json::to_string(&request_data).unwrap()
    });
    
    // Send and await response
    let response = Request::new()
        .target(target)
        .body(serde_json::to_vec(&wrapper).unwrap())
        .expects_response(30)  // 30 second timeout
        .send_and_await_response(30)
        .map_err(|e| format!("Remote request failed: {:?}", e))?;
    
    // Parse response
    if let Ok(body) = response.body() {
        Ok(String::from_utf8_lossy(&body).to_string())
    } else {
        Err("No response body".to_string())
    }
}

// On the receiving node
#[remote]
async fn get_data_since(&self, request_json: String) -> Result<String, String> {
    #[derive(Deserialize)]
    struct DataRequest {
        since: String,
        limit: usize,
    }
    
    let req: DataRequest = serde_json::from_str(&request_json)?;
    
    // Get requested data
    let data: Vec<_> = self.data.iter()
        .filter(|d| d.timestamp > req.since)
        .take(req.limit)
        .cloned()
        .collect();
    
    Ok(serde_json::to_string(&data).unwrap())
}
```

### Pattern 2: Fire-and-Forget Notifications

**Use Case:** Notify other nodes without waiting for response

```rust
// Broadcast notification to multiple nodes
#[http]
async fn broadcast_event(&mut self, request_body: String) -> Result<String, String> {
    #[derive(Deserialize)]
    struct BroadcastRequest {
        event_type: String,
        data: serde_json::Value,
    }
    
    let req: BroadcastRequest = serde_json::from_str(&request_body)?;
    
    let notification = json!({
        "event": req.event_type,
        "data": req.data,
        "from": our().node,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    
    let wrapper = json!({
        "HandleNotification": serde_json::to_string(&notification).unwrap()
    });
    
    let mut sent = 0;
    let mut failed = 0;
    
    // Send to all known nodes
    for node in &self.connected_nodes {
        let process_id = "skeleton-app:skeleton-app:skeleton.os".parse::<ProcessId>()?;
        let target = Address::new(node.clone(), process_id);
        
        // Fire and forget - still set timeout for reliability
        match Request::new()
            .target(target)
            .body(serde_json::to_vec(&wrapper).unwrap())
            .expects_response(5)  // Short timeout
            .send() {
                Ok(_) => sent += 1,
                Err(e) => {
                    println!("Failed to notify {}: {:?}", node, e);
                    failed += 1;
                }
            }
    }
    
    Ok(json!({
        "sent": sent,
        "failed": failed
    }).to_string())
}

// Receiving node
#[remote]
async fn handle_notification(&mut self, notification_json: String) -> Result<String, String> {
    let notification: serde_json::Value = serde_json::from_str(&notification_json)?;
    
    // Process notification
    self.notifications.push(notification);
    
    // Just acknowledge receipt
    Ok("ACK".to_string())
}
```

### Pattern 3: Distributed State Synchronization

**Use Case:** Keep state synchronized across multiple nodes

```rust
// State sync request
#[derive(Serialize, Deserialize)]
pub struct SyncRequest {
    pub node_id: String,
    pub state_hash: String,
    pub last_update: String,
}

#[derive(Serialize, Deserialize)]
pub struct SyncResponse {
    pub updates: Vec<StateUpdate>,
    pub full_sync_needed: bool,
}

// Periodic sync with peers
impl AppState {
    async fn sync_with_peer(&mut self, peer_node: String) -> Result<(), String> {
        let process_id = "skeleton-app:skeleton-app:skeleton.os".parse::<ProcessId>()?;
        let target = Address::new(peer_node.clone(), process_id);
        
        // Send our state info
        let sync_req = SyncRequest {
            node_id: our().node.clone(),
            state_hash: self.calculate_state_hash(),
            last_update: self.last_update_time.clone(),
        };
        
        let wrapper = json!({
            "HandleSyncRequest": serde_json::to_string(&sync_req).unwrap()
        });
        
        let response = Request::new()
            .target(target)
            .body(serde_json::to_vec(&wrapper).unwrap())
            .expects_response(30)
            .send_and_await_response(30)?;
        
        if let Ok(body) = response.body() {
            let sync_resp: SyncResponse = serde_json::from_slice(&body)?;
            
            if sync_resp.full_sync_needed {
                self.request_full_sync(peer_node).await?;
            } else {
                self.apply_updates(sync_resp.updates);
            }
        }
        
        Ok(())
    }
}

#[remote]
async fn handle_sync_request(&mut self, request_json: String) -> Result<String, String> {
    let req: SyncRequest = serde_json::from_str(&request_json)?;
    
    // Check if we have newer data
    let response = if req.state_hash != self.calculate_state_hash() {
        SyncResponse {
            updates: self.get_updates_since(&req.last_update),
            full_sync_needed: self.updates_since(&req.last_update) > 100,
        }
    } else {
        SyncResponse {
            updates: vec![],
            full_sync_needed: false,
        }
    };
    
    Ok(serde_json::to_string(&response).unwrap())
}
```

### Pattern 4: Collaborative Editing

**Use Case:** Multiple nodes editing shared data

```rust
// Operation-based CRDT pattern
#[derive(Serialize, Deserialize)]
pub enum Operation {
    Insert { pos: usize, text: String, id: String },
    Delete { pos: usize, len: usize, id: String },
    Update { item_id: String, field: String, value: serde_json::Value },
}

#[derive(Default, Serialize, Deserialize)]
pub struct SharedDocument {
    operations: Vec<Operation>,
    content: String,
    version: u64,
}

// Local edit creates operation
#[http]
async fn edit_document(&mut self, request_body: String) -> Result<String, String> {
    let op: Operation = serde_json::from_str(&request_body)?;
    
    // Apply locally
    self.document.apply_operation(&op);
    self.document.version += 1;
    
    // Broadcast to peers
    self.broadcast_operation(op).await?;
    
    Ok("Applied".to_string())
}

// Broadcast operation to all peers
impl AppState {
    async fn broadcast_operation(&self, op: Operation) -> Result<(), String> {
        let wrapper = json!({
            "ApplyOperation": serde_json::to_string(&op).unwrap()
        });
        
        let process_id = "skeleton-app:skeleton-app:skeleton.os".parse::<ProcessId>()?;
        
        for peer in &self.peers {
            let target = Address::new(peer.clone(), process_id);
            
            // Best effort delivery
            let _ = Request::new()
                .target(target)
                .body(serde_json::to_vec(&wrapper).unwrap())
                .expects_response(5)
                .send();
        }
        
        Ok(())
    }
}

// Receive operation from peer
#[remote]
async fn apply_operation(&mut self, op_json: String) -> Result<String, String> {
    let op: Operation = serde_json::from_str(&op_json)?;
    
    // Check if we've already seen this operation
    if !self.document.has_operation(&op) {
        self.document.apply_operation(&op);
        self.document.version += 1;
        
        // Forward to other peers (gossip protocol)
        self.broadcast_operation(op).await?;
    }
    
    Ok("Applied".to_string())
}
```

### Pattern 5: Node Authentication & Handshake

**Use Case:** Authenticate nodes before allowing access to resources

```rust
use hyperware_app_common::{send, source};

// Authentication request/response types
#[derive(Serialize, Deserialize)]
pub struct NodeHandshakeReq {
    pub resource_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct NodeHandshakeResp {
    pub auth_token: String,
}

// Client initiates handshake
#[http(method = "POST")]
async fn start_handshake(&mut self, url: String) -> Result<String, String> {
    // Extract node from URL (e.g., "https://bob.os/app/resource/123")
    let parts: Vec<&str> = url.split('/').collect();
    let host_node = parts.get(2)
        .ok_or("Invalid URL format")?
        .split(':').next()
        .ok_or("No host found")?;
    
    // Extract resource ID
    let resource_id = parts.last()
        .ok_or("No resource ID in URL")?
        .to_string();
    
    // Build target address
    let target = Address::new(host_node, ("app", "app", "publisher.os"));
    
    // Create handshake request
    let handshake_req = NodeHandshakeReq { resource_id };
    
    // Use typed send from hyperware_app_common
    let body = json!({"NodeHandshake": handshake_req});
    let request = Request::to(&target).body(serde_json::to_vec(&body).unwrap());
    
    match send::<Result<NodeHandshakeResp, String>>(request).await {
        Ok(Ok(resp)) => {
            // Store token and redirect with auth
            self.auth_tokens.insert(host_node.to_string(), resp.auth_token.clone());
            Ok(format!("{}?auth={}", url, resp.auth_token))
        }
        Ok(Err(e)) => Err(format!("Handshake failed: {}", e)),
        Err(e) => Err(format!("Request failed: {:?}", e)),
    }
}

// Server handles handshake - both local and remote calls
#[local]
#[remote]
async fn node_handshake(&mut self, req: NodeHandshakeReq) -> Result<NodeHandshakeResp, String> {
    // Verify resource exists
    if !self.resources.contains_key(&req.resource_id) {
        return Err("Resource not found".to_string());
    }
    
    // Get caller identity using source()
    let caller_node = source().node;
    
    // Generate unique auth token
    let auth_token = generate_auth_token();
    
    // Store token -> node mapping
    self.node_auth.insert(auth_token.clone(), NodeAuth {
        node_id: caller_node.clone(),
        resource_id: req.resource_id,
        granted_at: chrono::Utc::now().to_rfc3339(),
    });
    
    Ok(NodeHandshakeResp { auth_token })
}

// Verify token on subsequent requests
fn verify_auth(&self, token: &str) -> Result<NodeAuth, String> {
    self.node_auth.get(token)
        .cloned()
        .ok_or_else(|| "Invalid auth token".to_string())
}
```

### Pattern 6: Node Discovery & Presence

**Use Case:** Find and track active nodes

```rust
// Heartbeat/presence system
#[derive(Serialize, Deserialize)]
pub struct NodeInfo {
    pub node_id: String,
    pub app_version: String,
    pub capabilities: Vec<String>,
    pub last_seen: String,
}

// Announce presence to known nodes
impl AppState {
    async fn announce_presence(&self) -> Result<(), String> {
        let my_info = NodeInfo {
            node_id: our().node.clone(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            capabilities: vec!["sync".to_string(), "chat".to_string()],
            last_seen: chrono::Utc::now().to_rfc3339(),
        };
        
        let wrapper = json!({
            "RegisterNode": serde_json::to_string(&my_info).unwrap()
        });
        
        let process_id = "skeleton-app:skeleton-app:skeleton.os".parse::<ProcessId>()?;
        
        // Announce to bootstrap nodes
        for bootstrap in &self.bootstrap_nodes {
            let target = Address::new(bootstrap.clone(), process_id);
            
            match Request::new()
                .target(target)
                .body(serde_json::to_vec(&wrapper).unwrap())
                .expects_response(10)
                .send_and_await_response(10) {
                    Ok(response) => {
                        // Bootstrap node returns list of other nodes
                        if let Ok(body) = response.body() {
                            let nodes: Vec<NodeInfo> = serde_json::from_slice(&body)?;
                            self.discovered_nodes.extend(nodes);
                        }
                    },
                    Err(e) => println!("Bootstrap {} unreachable: {:?}", bootstrap, e),
                }
        }
        
        Ok(())
    }
}

#[remote]
async fn register_node(&mut self, info_json: String) -> Result<String, String> {
    let info: NodeInfo = serde_json::from_str(&info_json)?;
    
    // Update our node registry
    self.known_nodes.insert(info.node_id.clone(), info);
    
    // Return other known nodes
    let other_nodes: Vec<NodeInfo> = self.known_nodes.values()
        .filter(|n| n.node_id != info.node_id)
        .cloned()
        .collect();
    
    Ok(serde_json::to_string(&other_nodes).unwrap())
}
```

### Pattern 7: Distributed Transactions

**Use Case:** Coordinate actions across multiple nodes

```rust
// Two-phase commit pattern
#[derive(Serialize, Deserialize)]
pub enum TransactionPhase {
    Prepare,
    Commit,
    Abort,
}

#[derive(Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub operation: String,
    pub data: serde_json::Value,
    pub participants: Vec<String>,
}

// Coordinator node initiates transaction
#[http]
async fn start_transaction(&mut self, request_body: String) -> Result<String, String> {
    let mut tx: Transaction = serde_json::from_str(&request_body)?;
    tx.id = uuid::Uuid::new_v4().to_string();
    
    // Phase 1: Prepare
    let prepare_wrapper = json!({
        "PrepareTransaction": serde_json::to_string(&tx).unwrap()
    });
    
    let process_id = "skeleton-app:skeleton-app:skeleton.os".parse::<ProcessId>()?;
    let mut votes = HashMap::new();
    
    for participant in &tx.participants {
        let target = Address::new(participant.clone(), process_id);
        
        match Request::new()
            .target(target)
            .body(serde_json::to_vec(&prepare_wrapper).unwrap())
            .expects_response(10)
            .send_and_await_response(10) {
                Ok(response) => {
                    if let Ok(body) = response.body() {
                        let vote: bool = serde_json::from_slice(&body)?;
                        votes.insert(participant.clone(), vote);
                    }
                },
                Err(_) => {
                    votes.insert(participant.clone(), false);
                }
            }
    }
    
    // Phase 2: Commit or Abort
    let all_voted_yes = votes.values().all(|&v| v);
    let decision = if all_voted_yes { "Commit" } else { "Abort" };
    
    let decision_wrapper = json!({
        decision: tx.id.clone()
    });
    
    // Notify all participants of decision
    for participant in &tx.participants {
        let target = Address::new(participant.clone(), process_id);
        let _ = Request::new()
            .target(target)
            .body(serde_json::to_vec(&decision_wrapper).unwrap())
            .expects_response(5)
            .send();
    }
    
    Ok(json!({
        "transaction_id": tx.id,
        "decision": decision,
        "votes": votes,
    }).to_string())
}

// Participant node handlers
#[remote]
async fn prepare_transaction(&mut self, tx_json: String) -> Result<bool, String> {
    let tx: Transaction = serde_json::from_str(&tx_json)?;
    
    // Check if we can commit
    let can_commit = self.validate_transaction(&tx);
    
    if can_commit {
        // Save to pending
        self.pending_transactions.insert(tx.id.clone(), tx);
    }
    
    Ok(can_commit)
}

#[remote]
async fn commit(&mut self, tx_id: String) -> Result<String, String> {
    if let Some(tx) = self.pending_transactions.remove(&tx_id) {
        self.apply_transaction(tx)?;
        Ok("Committed".to_string())
    } else {
        Err("Transaction not found".to_string())
    }
}

#[remote]
async fn abort(&mut self, tx_id: String) -> Result<String, String> {
    self.pending_transactions.remove(&tx_id);
    Ok("Aborted".to_string())
}
```

## Error Handling & Resilience

### Retry with Exponential Backoff
```rust
// Note: This pattern requires the "timer:distro:sys" capability!
async fn reliable_remote_call(
    target: Address,
    method: &str,
    data: String,
) -> Result<String, String> {
    let wrapper = json!({ method: data });
    let body = serde_json::to_vec(&wrapper).unwrap();
    
    for attempt in 0..3 {
        if attempt > 0 {
            // Exponential backoff: 100ms, 200ms, 400ms
            let delay_ms = 100 * (1 << attempt);
            timer::set_timer(delay_ms, None);
        }
        
        match Request::new()
            .target(target.clone())
            .body(body.clone())
            .expects_response(30)
            .send_and_await_response(30) {
                Ok(response) => {
                    if let Ok(body) = response.body() {
                        return Ok(String::from_utf8_lossy(&body).to_string());
                    }
                },
                Err(e) if attempt < 2 => {
                    println!("Attempt {} failed: {:?}, retrying...", attempt + 1, e);
                    continue;
                },
                Err(e) => return Err(format!("Failed after 3 attempts: {:?}", e)),
            }
    }
    
    Err("Max retries exceeded".to_string())
}
```

### Circuit Breaker Pattern
```rust
// Note: HashMap is used here for internal state only - not exposed via WIT
#[derive(Default)]
pub struct CircuitBreaker {
    failures: HashMap<String, u32>,
    last_failure: HashMap<String, std::time::Instant>,
    threshold: u32,
    timeout_secs: u64,
}

impl CircuitBreaker {
    pub fn can_call(&self, node: &str) -> bool {
        if let Some(&failures) = self.failures.get(node) {
            if failures >= self.threshold {
                if let Some(&last) = self.last_failure.get(node) {
                    return last.elapsed().as_secs() > self.timeout_secs;
                }
            }
        }
        true
    }
    
    pub fn record_success(&mut self, node: &str) {
        self.failures.remove(node);
        self.last_failure.remove(node);
    }
    
    pub fn record_failure(&mut self, node: &str) {
        *self.failures.entry(node.to_string()).or_insert(0) += 1;
        self.last_failure.insert(node.to_string(), std::time::Instant::now());
    }
}
```

## Best Practices

### 1. Always Set Timeouts
```rust
// ✅ Good
.expects_response(30)
.send_and_await_response(30)

// ❌ Bad - Can hang forever
.send()
```

### 2. Handle Network Partitions
```rust
// Track node availability
pub struct NodeTracker {
    nodes: HashMap<String, NodeStatus>,
}

pub struct NodeStatus {
    last_successful_contact: String,
    consecutive_failures: u32,
    is_reachable: bool,
}
```

### 3. Use Idempotent Operations
```rust
// Include operation ID to prevent duplicates
#[derive(Serialize, Deserialize)]
pub struct Operation {
    pub id: String,  // Unique ID
    pub action: Action,
}

impl AppState {
    fn apply_operation(&mut self, op: Operation) -> Result<(), String> {
        // Check if already applied
        if self.applied_operations.contains(&op.id) {
            return Ok(()); // Idempotent
        }
        
        // Apply operation
        self.execute_action(op.action)?;
        self.applied_operations.insert(op.id);
        Ok(())
    }
}
```

### 4. Design for Eventual Consistency
```rust
// Use vector clocks or timestamps
#[derive(Serialize, Deserialize)]
pub struct VersionedData {
    pub data: serde_json::Value,
    pub version: VectorClock,
    pub last_modified: String,
}

// Resolve conflicts
impl VersionedData {
    fn merge(self, other: Self) -> Self {
        if self.version.happens_before(&other.version) {
            other
        } else if other.version.happens_before(&self.version) {
            self
        } else {
            // Concurrent updates - need resolution strategy
            self.resolve_conflict(other)
        }
    }
}
```

## Testing P2P Features

### Local Testing Setup
```bash
# Terminal 1
kit s --fake-node alice.os

# Terminal 2
kit s --fake-node bob.os

# Terminal 3 (optional)
kit s --fake-node charlie.os
```

### Test Scenarios
1. **Basic connectivity** - Can nodes find each other?
2. **Data sync** - Do all nodes eventually see the same data?
3. **Partition tolerance** - What happens when a node goes offline?
4. **Conflict resolution** - How are concurrent updates handled?
5. **Performance** - How does latency affect the user experience?

### Debug Output
```rust
// Add comprehensive logging
println!("[P2P] Sending {} to {}", method, target_node);
println!("[P2P] Response: {:?}", response);
println!("[P2P] State after sync: {:?}", self.state);
```

## Real-World P2P Patterns from samchat

### Pattern 8: Group Membership Notifications

**Use Case:** Notify all members when a group is created or modified

```rust
// Create group and notify all members
#[http]
async fn create_group(&mut self, group_name: String, initial_members: Vec<String>) -> Result<String, String> {
    let creator = self.my_node_id.clone()
        .ok_or_else(|| "Creator node ID not initialized".to_string())?;
    
    // Ensure creator is included
    let mut participants = initial_members;
    if !participants.contains(&creator) {
        participants.push(creator.clone());
    }
    
    let group_id = format!("group_{}", Uuid::new_v4());
    
    // Create group locally
    let conversation = Conversation {
        id: group_id.clone(),
        participants: participants.clone(),
        is_group: true,
        group_name: Some(group_name.clone()),
        created_by: Some(creator.clone()),
        // ...
    };
    self.conversations.insert(group_id.clone(), conversation);
    
    // Notify all other members
    let publisher = "hpn-testing-beta.os"; // Consistent across all nodes!
    let target_process_id = format!("samchat:samchat:{}", publisher)
        .parse::<ProcessId>()?;
    
    for participant in &participants {
        if participant != &creator {
            let target_address = Address::new(participant.clone(), target_process_id.clone());
            let notification = GroupJoinNotification {
                group_id: group_id.clone(),
                group_name: group_name.clone(),
                participants: participants.clone(),
                created_by: creator.clone(),
            };
            
            let request_wrapper = json!({ "HandleGroupJoin": notification });
            
            // Fire-and-forget but still set expects_response for reliability
            let _ = Request::new()
                .target(target_address)
                .body(serde_json::to_vec(&request_wrapper).unwrap())
                .expects_response(30)
                .send();
        }
    }
    
    Ok(group_id)
}

// Handle notification on receiving nodes
#[remote]
async fn handle_group_join(&mut self, notification: GroupJoinNotification) -> Result<bool, String> {
    // Create the group conversation locally
    let conversation = Conversation {
        id: notification.group_id.clone(),
        participants: notification.participants,
        is_group: true,
        group_name: Some(notification.group_name),
        created_by: Some(notification.created_by),
        // ...
    };
    
    self.conversations.insert(notification.group_id, conversation);
    Ok(true)
}
```

### Pattern 9: Remote Data Retrieval with Local Caching

**Use Case:** Fetch files or data from remote nodes with fallback

```rust
// Try local first, then remote
#[http]
async fn download_file(&mut self, file_id: String, sender_node: String) -> Result<Vec<u8>, String> {
    // Try local VFS first
    let file_path = format!("/samchat:hpn-testing-beta.os/files/{}", file_id);
    let vfs_address = Address::new(our().node.clone(), "vfs:distro:sys".parse::<ProcessId>()?);
    
    let local_result = Request::new()
        .target(vfs_address.clone())
        .body(json!({ "path": file_path, "action": "Read" }))
        .expects_response(5)
        .send_and_await_response(5);
    
    if let Ok(response) = local_result {
        if let Some(blob) = response.blob() {
            return Ok(blob.bytes);
        }
    }
    
    // Not found locally, fetch from remote
    if sender_node != our().node {
        let target = Address::new(sender_node, 
            "samchat:samchat:hpn-testing-beta.os".parse::<ProcessId>()?);
        
        let remote_result = Request::new()
            .target(target)
            .body(json!({ "GetRemoteFile": file_id }))
            .expects_response(30)
            .send_and_await_response(30)?;
        
        // Parse nested Result from remote
        let response_json: serde_json::Value = 
            serde_json::from_slice(&remote_result.body())?;
        
        if let Some(file_data) = response_json.get("Ok") {
            let bytes: Vec<u8> = serde_json::from_value(file_data.clone())?;
            
            // Cache locally for future use
            let _ = Request::new()
                .target(vfs_address)
                .body(json!({ "path": file_path, "action": "Write" }))
                .blob(LazyLoadBlob::new(Some("file"), bytes.clone()))
                .expects_response(5)
                .send_and_await_response(5);
            
            return Ok(bytes);
        }
    }
    
    Err("File not found".to_string())
}
```

### Pattern 10: Message Distribution to Multiple Recipients

**Use Case:** Send messages to group members without blocking

```rust
// Distribute message to all group members
async fn send_group_message(&mut self, group_id: String, content: String) -> Result<bool, String> {
    let sender = self.my_node_id.clone()
        .ok_or_else(|| "Node ID not initialized".to_string())?;
    
    let conversation = self.conversations.get(&group_id)
        .ok_or_else(|| "Group not found".to_string())?;
    
    // Get all recipients except sender
    let recipients: Vec<String> = conversation.participants.iter()
        .filter(|p| *p != &sender)
        .cloned()
        .collect();
    
    let message = ChatMessage {
        id: Uuid::new_v4().to_string(),
        conversation_id: group_id,
        sender,
        recipients: Some(recipients.clone()),
        content,
        timestamp: Utc::now().to_rfc3339(),
        // ...
    };
    
    // Save locally first
    self.conversations.get_mut(&group_id).unwrap()
        .messages.push(message.clone());
    
    // Distribute to all recipients
    let target_process_id = "samchat:samchat:hpn-testing-beta.os"
        .parse::<ProcessId>()?;
    
    for recipient in recipients {
        let target = Address::new(recipient, target_process_id.clone());
        
        // Fire-and-forget pattern but WITH expects_response
        let _ = Request::new()
            .target(target)
            .body(json!({ "ReceiveMessage": message.clone() }))
            .expects_response(30)  // Still set timeout!
            .send(); // Don't await response
    }
    
    Ok(true)
}
```

### Key Patterns from samchat:

1. **Consistent Publisher**: Always use the same publisher across all nodes
   ```rust
   let publisher = "hpn-testing-beta.os"; // Same for ALL nodes!
   let process_id = format!("samchat:samchat:{}", publisher);
   ```

2. **Fire-and-Forget WITH Timeout**: Even when not awaiting responses, set expects_response
   ```rust
   Request::new()
       .expects_response(30)  // Important for reliability
       .send();  // Not awaiting
   ```

3. **Node ID in State**: Store your node ID at initialization
   ```rust
   #[init]
   async fn initialize(&mut self) {
       self.my_node_id = Some(our().node.clone());
   }
   ```

4. **Optional Fields for Flexibility**: Use Option<T> for fields that vary by message type
   ```rust
   pub struct ChatMessage {
       recipient: Option<String>,      // Direct messages
       recipients: Option<Vec<String>>, // Group messages
       file_info: Option<FileInfo>,     // File attachments
       reply_to: Option<MessageReplyInfo>, // Replies
   }
   ```

## Remember

1. **No central authority** - Design for peer equality
2. **Expect failures** - Networks are unreliable
3. **Plan for conflicts** - Concurrent updates will happen
4. **Test with multiple nodes** - Single node testing misses P2P issues
5. **Document protocols** - Other developers need to understand your P2P design
6. **Consistent naming** - Use the same publisher/process names across all nodes
7. **Always set timeouts** - Even for fire-and-forget patterns