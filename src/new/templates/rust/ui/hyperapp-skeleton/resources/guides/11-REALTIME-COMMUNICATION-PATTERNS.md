# 🎙️ Real-time Communication Patterns Guide

## Overview

This guide covers advanced patterns for building real-time communication apps (voice chat, video calls, collaborative editing) in Hyperware. These patterns go beyond basic WebSockets to handle complex state synchronization, audio/video streaming, and participant management.

## Table of Contents

1. [WebSocket Protocol Design](#websocket-protocol-design)
2. [State Management for Real-time Apps](#state-management-for-realtime-apps)
3. [Audio/Video Streaming Patterns](#audiovideo-streaming-patterns)
4. [Participant Management](#participant-management)
5. [Dynamic UI Serving](#dynamic-ui-serving)
6. [Performance & Scalability](#performance--scalability)

## WebSocket Protocol Design

### Enum-based Message Protocol

Instead of string-based messages, use strongly-typed enums:

```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WSMessage {
    // Client -> Server
    Join { room_id: String, auth_token: Option<String> },
    Leave,
    SendData { content: String },
    UpdateState { key: String, value: serde_json::Value },
    
    // Server -> Client
    JoinSuccess { 
        user_id: String,
        room_state: RoomState,
        participants: Vec<Participant>,
    },
    JoinError { reason: String },
    StateUpdate { updates: HashMap<String, serde_json::Value> },
    ParticipantJoined { participant: Participant },
    ParticipantLeft { user_id: String },
    DataReceived { from_id: String, content: String },
}
```

### WebSocket Handler Pattern

```rust
#[ws]
fn handle_ws(&mut self, channel_id: u32, message_type: WsMessageType, payload: LazyLoadBlob) {
    match message_type {
        WsMessageType::Open => {
            // Track connection but don't add to room yet
            self.ws_connections.insert(channel_id, ConnectionState::Connected);
        }
        WsMessageType::Close => {
            self.handle_disconnect(channel_id);
        }
        WsMessageType::Message => {
            if let Ok(text) = String::from_utf8(payload.bytes) {
                if let Ok(msg) = serde_json::from_str::<WSMessage>(&text) {
                    self.handle_ws_message(channel_id, msg);
                }
            }
        }
        WsMessageType::Error => {
            self.handle_ws_error(channel_id);
        }
    }
}

fn handle_ws_message(&mut self, channel_id: u32, msg: WSMessage) {
    match msg {
        WSMessage::Join { room_id, auth_token } => {
            self.handle_join(channel_id, room_id, auth_token);
        }
        WSMessage::SendData { content } => {
            if let Some(user_id) = self.get_user_for_channel(channel_id) {
                self.broadcast_data(user_id, content);
            }
        }
        // Handle other message types...
    }
}
```

### Sending Messages

```rust
fn send_to_channel(&self, channel_id: u32, msg: WSMessage) {
    if let Ok(json) = serde_json::to_string(&msg) {
        get_server().send_ws_push(
            channel_id,
            WsMessageType::Text,
            LazyLoadBlob::new(Some("message"), json.into_bytes())
        );
    }
}

fn broadcast_to_room(&self, room_id: &str, msg: WSMessage, exclude: Option<&str>) {
    if let Some(room) = self.rooms.get(room_id) {
        for participant in &room.participants {
            if exclude.map_or(true, |ex| ex != participant.id) {
                if let Some(channel) = self.get_channel_for_user(&participant.id) {
                    self.send_to_channel(channel, msg.clone());
                }
            }
        }
    }
}
```

## State Management for Real-time Apps

### Connection State Tracking

```rust
#[derive(Default)]
pub struct RealTimeState {
    // WebSocket connections
    ws_connections: HashMap<u32, String>,  // channel_id -> user_id
    user_channels: HashMap<String, u32>,   // user_id -> channel_id
    
    // Room/session state
    rooms: HashMap<String, Room>,
    user_rooms: HashMap<String, String>,   // user_id -> room_id
    
    // Authentication
    auth_tokens: HashMap<String, AuthInfo>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Room {
    pub id: String,
    pub host_id: String,
    pub participants: Vec<Participant>,
    pub settings: RoomSettings,
    pub created_at: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Participant {
    pub id: String,
    pub display_name: String,
    pub role: ParticipantRole,
    pub joined_at: String,
    pub state: ParticipantState,
}
```

### Handling Participant State

```rust
impl RealTimeState {
    fn handle_join(&mut self, channel_id: u32, room_id: String, auth_token: Option<String>) {
        // Validate room exists
        if !self.rooms.contains_key(&room_id) {
            self.send_to_channel(channel_id, WSMessage::JoinError { 
                reason: "Room not found".to_string() 
            });
            return;
        }
        
        // Create participant
        let user_id = self.generate_user_id();
        let participant = Participant {
            id: user_id.clone(),
            display_name: format!("User{}", channel_id),
            role: self.determine_role(auth_token),
            joined_at: chrono::Utc::now().to_rfc3339(),
            state: ParticipantState::default(),
        };
        
        // Update state
        self.ws_connections.insert(channel_id, user_id.clone());
        self.user_channels.insert(user_id.clone(), channel_id);
        self.user_rooms.insert(user_id.clone(), room_id.clone());
        
        // Add to room
        if let Some(room) = self.rooms.get_mut(&room_id) {
            room.participants.push(participant.clone());
            
            // Send success to joiner
            self.send_to_channel(channel_id, WSMessage::JoinSuccess {
                user_id: user_id.clone(),
                room_state: self.get_room_state(&room_id),
                participants: room.participants.clone(),
            });
            
            // Notify others
            self.broadcast_to_room(&room_id, WSMessage::ParticipantJoined {
                participant,
            }, Some(&user_id));
        }
    }
    
    fn handle_disconnect(&mut self, channel_id: u32) {
        if let Some(user_id) = self.ws_connections.remove(&channel_id) {
            self.user_channels.remove(&user_id);
            
            if let Some(room_id) = self.user_rooms.remove(&user_id) {
                // Remove from room
                if let Some(room) = self.rooms.get_mut(&room_id) {
                    room.participants.retain(|p| p.id != user_id);
                    
                    // Notify others
                    self.broadcast_to_room(&room_id, WSMessage::ParticipantLeft {
                        user_id: user_id.clone(),
                    }, None);
                }
            }
        }
    }
}
```

## Audio/Video Streaming Patterns

### Audio Data Message Format

```rust
#[derive(Serialize, Deserialize)]
pub struct AudioData {
    pub data: String,        // Base64 encoded audio
    pub sequence: u32,       // For ordering and loss detection
    pub timestamp: u64,      // For jitter buffer
    pub sample_rate: u32,
    pub channels: u8,
}

// In your WebSocket handler
WSMessage::AudioData(audio) => {
    if let Some(user_id) = self.get_user_for_channel(channel_id) {
        if self.can_send_audio(&user_id) {
            self.distribute_audio(&user_id, audio);
        }
    }
}
```

### Mix-Minus Audio Distribution

For voice chat, each participant should hear everyone except themselves:

```rust
fn distribute_audio(&self, sender_id: &str, audio: AudioData) {
    if let Some(room_id) = self.user_rooms.get(sender_id) {
        if let Some(room) = self.rooms.get(room_id) {
            // Create personalized mix for each participant
            for participant in &room.participants {
                if participant.id != sender_id {  // Mix-minus
                    if let Some(channel) = self.get_channel_for_user(&participant.id) {
                        self.send_to_channel(channel, WSMessage::AudioStream {
                            from_id: sender_id.to_string(),
                            data: audio.clone(),
                        });
                    }
                }
            }
        }
    }
}
```

### Frontend Audio Handling

```typescript
// Audio service with jitter buffer
class AudioService {
  private jitterBuffers: Map<string, JitterBuffer> = new Map();
  private audioContext: AudioContext;
  
  async handleIncomingAudio(fromId: string, audioData: AudioData) {
    // Get or create jitter buffer for this participant
    let buffer = this.jitterBuffers.get(fromId);
    if (!buffer) {
      buffer = new JitterBuffer(this.audioContext);
      this.jitterBuffers.set(fromId, buffer);
    }
    
    // Decode and queue audio
    const decoded = await this.decodeAudio(audioData.data);
    buffer.push({
      sequence: audioData.sequence,
      timestamp: audioData.timestamp,
      samples: decoded,
    });
  }
}

// Jitter buffer implementation
class JitterBuffer {
  private buffer: Map<number, AudioPacket> = new Map();
  private nextSequence: number = 0;
  private targetDelay: number = 40; // ms
  
  push(packet: AudioPacket) {
    this.buffer.set(packet.sequence, packet);
    this.schedulePlayback();
  }
  
  private schedulePlayback() {
    // Play packets in order with proper timing
    while (this.buffer.has(this.nextSequence)) {
      const packet = this.buffer.get(this.nextSequence)!;
      this.playPacket(packet);
      this.buffer.delete(this.nextSequence);
      this.nextSequence++;
    }
  }
}
```

## Participant Management

### Role-based Access Control

```rust
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum ParticipantRole {
    Host,       // Can manage room settings, kick users
    Moderator,  // Can mute others, manage chat
    Speaker,    // Can send audio/video
    Viewer,     // Can only receive
}

impl RealTimeState {
    fn can_send_audio(&self, user_id: &str) -> bool {
        if let Some(room_id) = self.user_rooms.get(user_id) {
            if let Some(room) = self.rooms.get(room_id) {
                if let Some(participant) = room.participants.iter().find(|p| p.id == user_id) {
                    return matches!(participant.role, ParticipantRole::Host | ParticipantRole::Speaker);
                }
            }
        }
        false
    }
    
    fn can_manage_room(&self, user_id: &str) -> bool {
        // Similar check for Host/Moderator roles
    }
}
```

### Presence and Heartbeat

```rust
// Send periodic heartbeats to detect stale connections
fn start_heartbeat(&mut self) {
    // In practice, use timer capability
    // This is pseudo-code for the pattern
    every_30_seconds {
        for (channel_id, last_seen) in &self.connection_heartbeats {
            if now() - last_seen > 60 {
                self.handle_disconnect(*channel_id);
            }
        }
    }
}

// Client sends heartbeat
WSMessage::Heartbeat => {
    self.connection_heartbeats.insert(channel_id, now());
}
```

## Dynamic UI Serving

### Serving UI for Dynamic Routes

```rust
// When creating a new room/session
#[http]
async fn create_room(&mut self, request_body: String) -> Result<String, String> {
    let room = Room {
        id: generate_room_id(),
        host_id: get_caller_id(),
        participants: vec![],
        settings: Default::default(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    
    let room_id = room.id.clone();
    self.rooms.insert(room_id.clone(), room);
    
    // Serve UI at dynamic path
    get_server().serve_ui(
        &our(),
        format!("/room/{}", room_id),  // Dynamic path
        vec![],
        None,
        true
    )?;
    
    Ok(json!({ "room_id": room_id }).to_string())
}
```

### Frontend Routing

```typescript
// React router setup for dynamic paths
function App() {
  return (
    <Router>
      <Routes>
        <Route path="/" element={<HomePage />} />
        <Route path="/room/:roomId" element={<RoomView />} />
      </Routes>
    </Router>
  );
}

// Extract room ID and join
function RoomView() {
  const { roomId } = useParams();
  const store = useStore();
  
  useEffect(() => {
    store.joinRoom(roomId);
  }, [roomId]);
  
  return <RoomInterface />;
}
```

## Performance & Scalability

### Connection Pooling

```rust
// Limit connections per user/IP
#[derive(Default)]
struct ConnectionLimits {
    per_user: HashMap<String, usize>,
    per_ip: HashMap<String, usize>,
    max_per_user: usize,
    max_per_ip: usize,
}

impl ConnectionLimits {
    fn can_connect(&self, user_id: &str, ip: &str) -> bool {
        let user_count = self.per_user.get(user_id).unwrap_or(&0);
        let ip_count = self.per_ip.get(ip).unwrap_or(&0);
        
        *user_count < self.max_per_user && *ip_count < self.max_per_ip
    }
}
```

### Message Rate Limiting

```rust
#[derive(Default)]
struct RateLimiter {
    message_counts: HashMap<String, VecDeque<Instant>>,
    max_messages_per_minute: usize,
}

impl RateLimiter {
    fn check_rate_limit(&mut self, user_id: &str) -> bool {
        let now = Instant::now();
        let counts = self.message_counts.entry(user_id.to_string())
            .or_insert_with(VecDeque::new);
        
        // Remove old entries
        while let Some(&front) = counts.front() {
            if now.duration_since(front) > Duration::from_secs(60) {
                counts.pop_front();
            } else {
                break;
            }
        }
        
        if counts.len() < self.max_messages_per_minute {
            counts.push_back(now);
            true
        } else {
            false
        }
    }
}
```

### Efficient State Updates

```rust
// Batch state updates
struct StateBatcher {
    pending_updates: HashMap<String, Vec<StateUpdate>>,
    flush_interval: Duration,
}

impl StateBatcher {
    fn add_update(&mut self, room_id: String, update: StateUpdate) {
        self.pending_updates.entry(room_id)
            .or_insert_with(Vec::new)
            .push(update);
    }
    
    fn flush(&mut self) {
        for (room_id, updates) in self.pending_updates.drain() {
            if !updates.is_empty() {
                self.broadcast_to_room(&room_id, WSMessage::BatchedUpdates {
                    updates,
                }, None);
            }
        }
    }
}
```

## Best Practices

### 1. Message Protocol Design
- Use strongly-typed enums with serde tags
- Version your protocol for backwards compatibility
- Keep messages small and focused
- Use compression for large payloads

### 2. State Management
- Minimize state on the server
- Use immutable updates where possible
- Clean up disconnected users promptly
- Implement proper session recovery

### 3. Audio/Video Handling
- Implement jitter buffers for smooth playback
- Use appropriate codecs (Opus for audio)
- Handle packet loss gracefully
- Monitor quality metrics

### 4. Security
- Authenticate WebSocket connections
- Validate all incoming messages
- Rate limit by user and IP
- Sanitize user-generated content
- Use secure random IDs for rooms/sessions

### 5. Scalability
- Design for horizontal scaling
- Use efficient data structures
- Implement connection limits
- Monitor resource usage
- Plan for graceful degradation

## Example: Minimal Voice Chat

Here's a complete minimal example combining these patterns:

```rust
use hyperprocess_macro::*;
use hyperware_process_lib::{
    our, bindings::get_server, 
    WsMessageType, LazyLoadBlob
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum VoiceMessage {
    // Client -> Server
    Join { room_id: String },
    Leave,
    AudioData { data: String, seq: u32 },
    
    // Server -> Client  
    Joined { user_id: String },
    UserJoined { user_id: String },
    UserLeft { user_id: String },
    AudioStream { from: String, data: String, seq: u32 },
}

#[derive(Default, Serialize, Deserialize)]
struct VoiceChat {
    rooms: HashMap<String, Vec<String>>,  // room_id -> user_ids
    connections: HashMap<u32, String>,    // channel -> user_id
    users: HashMap<String, String>,       // user_id -> room_id
}

#[hyperprocess(
    name = "Voice Chat",
    endpoints = vec![
        Binding::WebSocket {
            path: "/ws",
            config: HttpBindingConfig::new(false, false, false, None)
        }
    ],
)]
impl VoiceChat {
    #[ws]
    fn handle_ws(&mut self, channel: u32, msg_type: WsMessageType, payload: LazyLoadBlob) {
        if msg_type == WsMessageType::Message {
            if let Ok(text) = String::from_utf8(payload.bytes) {
                if let Ok(msg) = serde_json::from_str::<VoiceMessage>(&text) {
                    match msg {
                        VoiceMessage::Join { room_id } => {
                            let user_id = format!("user-{}", channel);
                            
                            // Add to room
                            self.rooms.entry(room_id.clone())
                                .or_default()
                                .push(user_id.clone());
                            self.connections.insert(channel, user_id.clone());
                            self.users.insert(user_id.clone(), room_id.clone());
                            
                            // Send joined confirmation
                            self.send_ws(channel, VoiceMessage::Joined { 
                                user_id: user_id.clone() 
                            });
                            
                            // Notify others
                            self.broadcast_to_room(&room_id, VoiceMessage::UserJoined { 
                                user_id 
                            }, Some(channel));
                        }
                        VoiceMessage::AudioData { data, seq } => {
                            if let Some(user_id) = self.connections.get(&channel) {
                                if let Some(room_id) = self.users.get(user_id) {
                                    // Send audio to everyone else in room
                                    self.broadcast_to_room(room_id, VoiceMessage::AudioStream {
                                        from: user_id.clone(),
                                        data,
                                        seq,
                                    }, Some(channel));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    
    fn send_ws(&self, channel: u32, msg: VoiceMessage) {
        if let Ok(json) = serde_json::to_string(&msg) {
            get_server().send_ws_push(
                channel,
                WsMessageType::Text,
                LazyLoadBlob::new(Some("msg"), json.into_bytes())
            );
        }
    }
    
    fn broadcast_to_room(&self, room_id: &str, msg: VoiceMessage, exclude: Option<u32>) {
        if let Some(users) = self.rooms.get(room_id) {
            for user_id in users {
                for (ch, uid) in &self.connections {
                    if uid == user_id && exclude.map_or(true, |ex| ex != *ch) {
                        self.send_ws(*ch, msg.clone());
                    }
                }
            }
        }
    }
}
```

## P2P Real-time Patterns

### Polling-based Updates

For P2P applications where WebSockets aren't needed, polling provides a simpler alternative:

```typescript
// Frontend polling pattern
const POLLING_INTERVAL = 5000; // 5 seconds

useEffect(() => {
  // Initial fetch
  fetchData();
  
  // Set up polling
  const intervalId = setInterval(fetchData, POLLING_INTERVAL);
  
  return () => clearInterval(intervalId);
}, [fetchData]);
```

### P2P Message Distribution

Instead of WebSocket broadcasting, P2P apps distribute messages via the Request API:

```rust
// Send message to multiple recipients
for recipient in recipients {
    let target_address = Address::new(recipient.clone(), target_process_id.clone());
    let request_wrapper = json!({
        "ReceiveMessage": message.clone()
    });
    
    // Fire-and-forget pattern for real-time distribution
    Request::new()
        .target(target_address)
        .body(serde_json::to_vec(&request_wrapper)?)
        .expects_response(30)  // Still set timeout
        .send();  // Don't await
}
```

### State Synchronization via HTTP

P2P apps can achieve real-time feel through efficient HTTP endpoints:

```rust
#[http]
async fn get_updates(&self, request_body: String) -> Result<String, String> {
    #[derive(Deserialize)]
    struct UpdateRequest {
        last_update_timestamp: String,
    }
    
    let req: UpdateRequest = serde_json::from_str(&request_body)?;
    
    // Return only changes since last update
    let updates = self.get_changes_since(&req.last_update_timestamp);
    Ok(serde_json::to_string(&updates)?)
}
```

### Choosing Between WebSockets and Polling

**Use WebSockets when:**
- Sub-second latency is critical (voice/video)
- High frequency updates (collaborative editing)
- Server needs to push data immediately
- Bandwidth efficiency is important

**Use Polling when:**
- Updates can tolerate 5-10 second delays
- Implementation simplicity is valued
- P2P architecture without central coordination
- Resilience to connection issues is important

## Conclusion

Real-time communication in Hyperware can be achieved through:
- **WebSocket patterns** for low-latency, high-frequency updates
- **P2P polling patterns** for simpler, resilient architectures
- **Hybrid approaches** combining both as needed

Choose the pattern that best fits your application's requirements for latency, complexity, and scalability.