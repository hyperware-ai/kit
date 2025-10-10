# 🧪 Testing & Debugging Guide

## Development Environment Setup

### 1. Local Node Testing
```bash
# Single node (default)
kit s

# Multiple local nodes for P2P testing
# Terminal 1
kit s --fake-node alice.os

# Terminal 2  
kit s --fake-node bob.os

# Terminal 3
kit s --fake-node charlie.os
```

### 2. Environment Variables
```bash
# Enable verbose logging
RUST_LOG=debug kit s

# Custom port (if default is taken)
kit s --port 8081
```

## Debugging Strategies

### 1. Backend Debugging (Rust)

#### Strategic println! Debugging
```rust
// Add context to all prints
impl AppState {
    fn debug_log(&self, context: &str, message: &str) {
        println!("[{}] {}: {}", 
            chrono::Utc::now().format("%H:%M:%S%.3f"),
            context,
            message
        );
    }
}

// Use in handlers
#[http]
async fn complex_operation(&mut self, request_body: String) -> Result<String, String> {
    self.debug_log("complex_operation", &format!("Request: {}", request_body));
    
    let parsed: MyRequest = serde_json::from_str(&request_body)
        .map_err(|e| {
            self.debug_log("complex_operation", &format!("Parse error: {}", e));
            format!("Invalid request: {}", e)
        })?;
    
    self.debug_log("complex_operation", &format!("Parsed: {:?}", parsed));
    
    // Operation logic...
    
    self.debug_log("complex_operation", "Operation completed successfully");
    Ok("Success".to_string())
}
```

#### State Inspection
```rust
// Add debug endpoint to inspect state
#[http]
async fn debug_state(&self, _request_body: String) -> String {
    // Only in development!
    if cfg!(debug_assertions) {
        serde_json::json!({
            "node": our().node,
            "item_count": self.items.len(),
            "connected_nodes": self.connected_nodes,
            "last_sync": self.last_sync_time,
            "pending_operations": self.pending_operations.len(),
            // Don't expose sensitive data
        }).to_string()
    } else {
        "Debug disabled in production".to_string()
    }
}

// Pretty print for complex debugging
#[http]
async fn debug_item(&self, request_body: String) -> String {
    let id: String = serde_json::from_str(&request_body).unwrap_or_default();
    
    if let Some(item) = self.items.iter().find(|i| i.id == id) {
        // Pretty print with indentation
        serde_json::to_string_pretty(item).unwrap()
    } else {
        "Not found".to_string()
    }
}
```

#### P2P Communication Debugging
```rust
// Wrap remote calls with debugging
async fn debug_remote_call(
    &self,
    target: Address,
    method: &str,
    data: String,
) -> Result<String, String> {
    println!("\n=== P2P DEBUG START ===");
    println!("Target: {:?}", target);
    println!("Method: {}", method);
    println!("Request: {}", data);
    
    let start = std::time::Instant::now();
    let wrapper = json!({ method: data });
    
    let result = Request::new()
        .target(target)
        .body(serde_json::to_vec(&wrapper).unwrap())
        .expects_response(30)
        .send_and_await_response(30);
    
    let duration = start.elapsed();
    
    match &result {
        Ok(response) => {
            if let Ok(body) = response.body() {
                let body_str = String::from_utf8_lossy(&body);
                println!("Response ({}ms): {}", duration.as_millis(), body_str);
            }
        }
        Err(e) => {
            println!("Error ({}ms): {:?}", duration.as_millis(), e);
        }
    }
    
    println!("=== P2P DEBUG END ===\n");
    
    result.map(|r| String::from_utf8_lossy(&r.body().unwrap_or_default()).to_string())
        .map_err(|e| format!("{:?}", e))
}
```

### 2. Frontend Debugging (React/TypeScript)

#### API Call Debugging
```typescript
// src/utils/debug.ts
const DEBUG = import.meta.env.DEV;

export function debugLog(category: string, ...args: any[]) {
  if (DEBUG) {
    console.log(`[${new Date().toISOString()}] [${category}]`, ...args);
  }
}

// Enhanced API wrapper with debugging
export async function debugApiCall<T>(
  method: string,
  data: any,
  description: string
): Promise<T> {
  const requestId = Math.random().toString(36).substr(2, 9);
  
  debugLog('API', `[${requestId}] Starting: ${description}`);
  debugLog('API', `[${requestId}] Method: ${method}`);
  debugLog('API', `[${requestId}] Data:`, data);
  
  const startTime = performance.now();
  
  try {
    const result = await makeApiCall<any, T>(method, data);
    const duration = performance.now() - startTime;
    
    debugLog('API', `[${requestId}] Success (${duration.toFixed(2)}ms):`, result);
    
    return result;
  } catch (error) {
    const duration = performance.now() - startTime;
    debugLog('API', `[${requestId}] Error (${duration.toFixed(2)}ms):`, error);
    throw error;
  }
}
```

#### State Debugging with Zustand
```typescript
// src/store/debug.ts
import { useAppStore } from './app';

// Debug middleware
export const debugMiddleware = (config: any) => (set: any, get: any, api: any) =>
  config(
    (args: any) => {
      console.log('  [State Change]', args);
      set(args);
    },
    get,
    api
  );

// Usage in store
export const useAppStore = create<AppState>()(
  devtools(
    debugMiddleware((set, get) => ({
      // Your store implementation
    })),
    {
      name: 'app-store',
    }
  )
);

// Debug component
export const StoreDebugger: React.FC = () => {
  const store = useAppStore();
  
  if (!import.meta.env.DEV) return null;
  
  return (
    <div style={{
      position: 'fixed',
      bottom: 0,
      right: 0,
      background: 'black',
      color: 'white',
      padding: '10px',
      maxWidth: '300px',
      maxHeight: '200px',
      overflow: 'auto',
      fontSize: '12px',
      fontFamily: 'monospace',
    }}>
      <h4>Store State</h4>
      <pre>{JSON.stringify(store, null, 2)}</pre>
    </div>
  );
};
```

#### React DevTools Integration
```typescript
// Name components for better debugging
export const ItemList = React.memo(
  function ItemList({ items }: { items: Item[] }) {
    // Component logic
  }
);

// Add display names to hooks
export function useItems() {
  // Hook logic
}
useItems.displayName = 'useItems';
```

### 3. WebSocket Debugging

#### Monitor WebSocket Traffic
```typescript
// src/utils/ws-debug.ts
class WebSocketDebugger {
  private originalWS = WebSocket;
  
  enable() {
    const self = this;
    // @ts-ignore
    window.WebSocket = class extends this.originalWS {
      constructor(url: string, protocols?: string | string[]) {
        console.log(`🔌 WS Connecting to: ${url}`);
        super(url, protocols);
        
        this.addEventListener('open', (event) => {
          console.log(`✅ WS Connected: ${url}`);
        });
        
        this.addEventListener('message', (event) => {
          try {
            const data = JSON.parse(event.data);
            console.log(`📨 WS Received:`, data);
          } catch {
            console.log(`📨 WS Received (raw):`, event.data);
          }
        });
        
        this.addEventListener('error', (event) => {
          console.error(`❌ WS Error:`, event);
        });
        
        this.addEventListener('close', (event) => {
          console.log(`🔌 WS Closed: code=${event.code} reason=${event.reason}`);
        });
        
        // Intercept send
        const originalSend = this.send.bind(this);
        this.send = (data: any) => {
          console.log(`📤 WS Send:`, typeof data === 'string' ? JSON.parse(data) : data);
          return originalSend(data);
        };
      }
    };
  }
  
  disable() {
    window.WebSocket = this.originalWS;
  }
}

export const wsDebugger = new WebSocketDebugger();

// Enable in development
if (import.meta.env.DEV) {
  wsDebugger.enable();
}
```

### 4. Network Debugging

#### Monitor All HTTP Traffic
```typescript
// src/utils/network-debug.ts
if (import.meta.env.DEV) {
  // Intercept fetch
  const originalFetch = window.fetch;
  window.fetch = async (...args) => {
    const [url, options] = args;
    console.group(`🌐 Fetch: ${options?.method || 'GET'} ${url}`);
    console.log('Request:', options);
    
    try {
      const response = await originalFetch(...args);
      const clone = response.clone();
      const data = await clone.json().catch(() => 'Non-JSON response');
      
      console.log('Response:', {
        status: response.status,
        statusText: response.statusText,
        data,
      });
      console.groupEnd();
      
      return response;
    } catch (error) {
      console.error('Error:', error);
      console.groupEnd();
      throw error;
    }
  };
}
```

## Testing Patterns

### 1. Unit Testing (Rust)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_state() -> AppState {
        AppState {
            items: vec![
                Item { id: "1".to_string(), name: "Test 1".to_string() },
                Item { id: "2".to_string(), name: "Test 2".to_string() },
            ],
            ..Default::default()
        }
    }
    
    #[test]
    fn test_item_creation() {
        let mut state = create_test_state();
        let initial_count = state.items.len();
        
        // Simulate item creation
        state.items.push(Item {
            id: "3".to_string(),
            name: "Test 3".to_string(),
        });
        
        assert_eq!(state.items.len(), initial_count + 1);
        assert_eq!(state.items.last().unwrap().name, "Test 3");
    }
    
    #[test]
    fn test_item_deletion() {
        let mut state = create_test_state();
        state.items.retain(|item| item.id != "1");
        
        assert_eq!(state.items.len(), 1);
        assert!(!state.items.iter().any(|i| i.id == "1"));
    }
    
    #[tokio::test]
    async fn test_async_operation() {
        let mut state = create_test_state();
        
        // Test async method
        let result = state.process_items().await;
        assert!(result.is_ok());
    }
}
```

### 2. Integration Testing

```typescript
// src/__tests__/integration.test.ts
import { renderHook, act, waitFor } from '@testing-library/react';
import { useAppStore } from '../store/app';
import * as api from '../utils/api';

// Mock API
jest.mock('../utils/api');

describe('App Integration', () => {
  beforeEach(() => {
    // Reset store
    useAppStore.setState({
      items: [],
      isLoading: false,
      error: null,
    });
  });
  
  it('fetches and displays items', async () => {
    const mockItems = [
      { id: '1', name: 'Item 1' },
      { id: '2', name: 'Item 2' },
    ];
    
    (api.getItems as jest.Mock).mockResolvedValue(mockItems);
    
    const { result } = renderHook(() => useAppStore());
    
    // Trigger fetch
    await act(async () => {
      await result.current.fetchItems();
    });
    
    await waitFor(() => {
      expect(result.current.items).toEqual(mockItems);
      expect(result.current.isLoading).toBe(false);
    });
  });
  
  it('handles errors gracefully', async () => {
    const error = new Error('Network error');
    (api.getItems as jest.Mock).mockRejectedValue(error);
    
    const { result } = renderHook(() => useAppStore());
    
    await act(async () => {
      await result.current.fetchItems();
    });
    
    expect(result.current.error).toBe('Network error');
    expect(result.current.items).toEqual([]);
  });
});
```

### 3. P2P Testing Scenarios

```bash
# test-p2p.sh
#!/bin/bash

echo "Starting P2P test environment..."

# Start nodes
kit s --fake-node alice.os &
ALICE_PID=$!

sleep 2

kit s --fake-node bob.os --port 8081 &
BOB_PID=$!

sleep 2

echo "Nodes started: Alice (PID: $ALICE_PID), Bob (PID: $BOB_PID)"
echo "Access Alice at http://localhost:8080"
echo "Access Bob at http://localhost:8081"

# Wait for user to finish testing
read -p "Press Enter to stop nodes..."

# Cleanup
kill $ALICE_PID $BOB_PID
echo "Test environment stopped"
```

#### P2P Test Checklist
```typescript
// src/tests/p2p-checklist.ts
export const P2P_TEST_SCENARIOS = [
  {
    name: "Basic Connectivity",
    steps: [
      "Start two nodes (alice.os and bob.os)",
      "From Alice, try to connect to Bob",
      "Verify connection status on both nodes",
      "Check console logs for any errors",
    ],
  },
  {
    name: "Data Synchronization",
    steps: [
      "Create data on Alice node",
      "Trigger sync from Bob node",
      "Verify data appears on Bob",
      "Modify data on Bob",
      "Sync back to Alice",
      "Verify both nodes have same data",
    ],
  },
  {
    name: "Network Resilience",
    steps: [
      "Establish connection between nodes",
      "Stop Bob node (Ctrl+C)",
      "Try operation from Alice",
      "Verify graceful error handling",
      "Restart Bob node",
      "Verify automatic reconnection",
    ],
  },
  {
    name: "Concurrent Updates",
    steps: [
      "Open same item on both nodes",
      "Make different changes simultaneously",
      "Save on both nodes",
      "Verify conflict resolution",
      "Check final state consistency",
    ],
  },
];
```

## Performance Profiling

### 1. Backend Performance
```rust
// Simple timing macro
macro_rules! time_operation {
    ($name:expr, $body:expr) => {{
        let start = std::time::Instant::now();
        let result = $body;
        let duration = start.elapsed();
        println!("[PERF] {} took {:?}", $name, duration);
        result
    }};
}

// Usage
#[http]
async fn heavy_operation(&mut self, request_body: String) -> Result<String, String> {
    let parsed = time_operation!("parsing", {
        serde_json::from_str::<ComplexRequest>(&request_body)?
    });
    
    let result = time_operation!("processing", {
        self.process_complex_request(parsed)?
    });
    
    time_operation!("serializing", {
        Ok(serde_json::to_string(&result).unwrap())
    })
}
```

### 2. Frontend Performance
```typescript
// src/utils/performance.ts
export class PerformanceMonitor {
  private marks: Map<string, number> = new Map();
  
  start(label: string) {
    this.marks.set(label, performance.now());
  }
  
  end(label: string, threshold = 100) {
    const start = this.marks.get(label);
    if (!start) return;
    
    const duration = performance.now() - start;
    this.marks.delete(label);
    
    if (duration > threshold) {
      console.warn(`⚠️ Slow operation: ${label} took ${duration.toFixed(2)}ms`);
    } else if (import.meta.env.DEV) {
      console.log(`✅ ${label}: ${duration.toFixed(2)}ms`);
    }
    
    return duration;
  }
}

export const perfMon = new PerformanceMonitor();

// Usage
const fetchData = async () => {
  perfMon.start('fetchData');
  try {
    const data = await api.getData();
    return data;
  } finally {
    perfMon.end('fetchData');
  }
};
```

## Common Debugging Scenarios

### 1. "Why isn't my endpoint being called?"
```typescript
// Debug checklist component
export const EndpointDebugger: React.FC = () => {
  const [results, setResults] = useState<any[]>([]);
  
  const runDiagnostics = async () => {
    const diagnostics = [];
    
    // Check 1: Node connection
    diagnostics.push({
      test: 'Node Connection',
      result: window.our ? `✅ Connected as ${window.our.node}` : '❌ No connection',
    });
    
    // Check 2: API endpoint
    try {
      const response = await fetch('/api');
      diagnostics.push({
        test: 'API Endpoint',
        result: `✅ Status ${response.status}`,
      });
    } catch (error) {
      diagnostics.push({
        test: 'API Endpoint',
        result: `❌ Error: ${error}`,
      });
    }
    
    // Check 3: Method call
    try {
      await api.getStatus();
      diagnostics.push({
        test: 'GetStatus Method',
        result: '✅ Success',
      });
    } catch (error) {
      diagnostics.push({
        test: 'GetStatus Method',
        result: `❌ Error: ${error}`,
      });
    }
    
    setResults(diagnostics);
  };
  
  return (
    <div className="debugger">
      <button onClick={runDiagnostics}>Run Diagnostics</button>
      <ul>
        {results.map((r, i) => (
          <li key={i}>{r.test}: {r.result}</li>
        ))}
      </ul>
    </div>
  );
};
```

### 2. "Why is my P2P call failing?"
```rust
// Diagnostic endpoint
#[http]
async fn diagnose_p2p(&self, request_body: String) -> String {
    let target_node: String = serde_json::from_str(&request_body).unwrap_or_default();
    let mut diagnostics = vec![];
    
    // Check 1: ProcessId parsing
    match "skeleton-app:skeleton-app:skeleton.os".parse::<ProcessId>() {
        Ok(pid) => diagnostics.push(format!("✅ ProcessId valid: {:?}", pid)),
        Err(e) => diagnostics.push(format!("❌ ProcessId error: {}", e)),
    }
    
    // Check 2: Address construction
    if !target_node.is_empty() {
        let pid = "skeleton-app:skeleton-app:skeleton.os".parse::<ProcessId>().ok();
        if let Some(pid) = pid {
            let addr = Address::new(target_node.clone(), pid);
            diagnostics.push(format!("✅ Address created: {:?}", addr));
            
            // Check 3: Ping attempt
            let ping = json!({ "Ping": "" });
            match Request::new()
                .target(addr)
                .body(serde_json::to_vec(&ping).unwrap())
                .expects_response(5)
                .send_and_await_response(5) {
                    Ok(_) => diagnostics.push("✅ Node reachable".to_string()),
                    Err(e) => diagnostics.push(format!("❌ Node unreachable: {:?}", e)),
                }
        }
    }
    
    serde_json::to_string(&diagnostics).unwrap()
}
```

## Audio Context Debugging

### Monitor Audio Context State
```typescript
// Track audio context states for debugging autoplay issues
export class AudioContextMonitor {
  private contexts: Map<string, AudioContext> = new Map();
  
  register(name: string, context: AudioContext): void {
    this.contexts.set(name, context);
    
    // Log initial state
    console.log(`[Audio] ${name} registered - state: ${context.state}`);
    
    // Monitor state changes
    context.addEventListener('statechange', () => {
      console.log(`[Audio] ${name} state changed to: ${context.state}`);
    });
  }
  
  async resumeAll(): Promise<void> {
    for (const [name, context] of this.contexts) {
      if (context.state === 'suspended') {
        try {
          await context.resume();
          console.log(`[Audio] ${name} resumed successfully`);
        } catch (error) {
          console.error(`[Audio] Failed to resume ${name}:`, error);
        }
      }
    }
  }
  
  getStates(): Record<string, AudioContextState> {
    const states: Record<string, AudioContextState> = {};
    for (const [name, context] of this.contexts) {
      states[name] = context.state;
    }
    return states;
  }
}

// Usage in your app
const audioMonitor = new AudioContextMonitor();
audioMonitor.register('playback', playbackContext);
audioMonitor.register('capture', captureContext);
```

## Real-time Performance Monitoring

### Track Critical Metrics
```typescript
// Monitor packet loss and jitter
class NetworkMetrics {
  private sequenceNumbers: Map<string, number> = new Map();
  private packetLoss: Map<string, number[]> = new Map();
  private jitterBuffer: Map<string, number[]> = new Map();
  
  trackPacket(streamId: string, sequence: number, timestamp: number): void {
    // Track sequence gaps
    const lastSeq = this.sequenceNumbers.get(streamId) || -1;
    if (lastSeq !== -1 && sequence > lastSeq + 1) {
      const losses = this.packetLoss.get(streamId) || [];
      for (let i = lastSeq + 1; i < sequence; i++) {
        losses.push(i);
      }
      this.packetLoss.set(streamId, losses);
    }
    this.sequenceNumbers.set(streamId, sequence);
    
    // Track jitter
    const jitters = this.jitterBuffer.get(streamId) || [];
    jitters.push(timestamp);
    if (jitters.length > 100) jitters.shift(); // Keep last 100
    this.jitterBuffer.set(streamId, jitters);
  }
  
  getStats(streamId: string): { loss: number; jitter: number } {
    const losses = this.packetLoss.get(streamId) || [];
    const total = this.sequenceNumbers.get(streamId) || 0;
    const lossRate = total > 0 ? losses.length / total : 0;
    
    const jitters = this.jitterBuffer.get(streamId) || [];
    const jitter = this.calculateJitter(jitters);
    
    return { loss: lossRate, jitter };
  }
  
  private calculateJitter(timestamps: number[]): number {
    if (timestamps.length < 2) return 0;
    let sum = 0;
    for (let i = 1; i < timestamps.length; i++) {
      sum += Math.abs(timestamps[i] - timestamps[i-1]);
    }
    return sum / (timestamps.length - 1);
  }
}
```

### Heartbeat Monitoring
```typescript
// Monitor connection health with heartbeats
class HeartbeatMonitor {
  private intervals: Map<string, NodeJS.Timeout> = new Map();
  private lastHeartbeats: Map<string, number> = new Map();
  private callbacks: Map<string, () => void> = new Map();
  
  start(id: string, intervalMs: number, timeoutMs: number, onTimeout: () => void): void {
    this.callbacks.set(id, onTimeout);
    this.lastHeartbeats.set(id, Date.now());
    
    const interval = setInterval(() => {
      const last = this.lastHeartbeats.get(id) || 0;
      const elapsed = Date.now() - last;
      
      if (elapsed > timeoutMs) {
        console.warn(`[Heartbeat] ${id} timed out after ${elapsed}ms`);
        this.stop(id);
        onTimeout();
      }
    }, intervalMs);
    
    this.intervals.set(id, interval);
  }
  
  pulse(id: string): void {
    this.lastHeartbeats.set(id, Date.now());
  }
  
  stop(id: string): void {
    const interval = this.intervals.get(id);
    if (interval) {
      clearInterval(interval);
      this.intervals.delete(id);
    }
  }
}
```

## Memory Management & Cleanup

### Periodic Resource Cleanup
```typescript
// Prevent memory leaks in long-running sessions
class ResourceManager {
  private cleanupTasks: Array<() => void> = [];
  private cleanupInterval: number | null = null;
  
  register(cleanup: () => void): void {
    this.cleanupTasks.push(cleanup);
  }
  
  startPeriodicCleanup(intervalMs: number = 60000): void {
    this.cleanupInterval = window.setInterval(() => {
      console.log('[ResourceManager] Running periodic cleanup...');
      
      // Run all cleanup tasks
      for (const task of this.cleanupTasks) {
        try {
          task();
        } catch (error) {
          console.error('[ResourceManager] Cleanup task failed:', error);
        }
      }
      
      // Hint for garbage collection (browser may ignore)
      if ('gc' in window && typeof (window as any).gc === 'function') {
        (window as any).gc();
      }
    }, intervalMs);
  }
  
  cleanup(): void {
    if (this.cleanupInterval !== null) {
      clearInterval(this.cleanupInterval);
      this.cleanupInterval = null;
    }
    
    // Run final cleanup
    for (const task of this.cleanupTasks) {
      try {
        task();
      } catch (error) {
        console.error('[ResourceManager] Final cleanup failed:', error);
      }
    }
  }
}

// Usage example
const resources = new ResourceManager();

// Register cleanup for audio buffers
resources.register(() => {
  // Clean up old jitter buffers
  for (const [key, buffer] of jitterBuffers) {
    if (buffer.getBufferSize() === 0 && Date.now() - buffer.lastActivity > 60000) {
      buffer.cleanup();
      jitterBuffers.delete(key);
    }
  }
});

resources.startPeriodicCleanup();
```

## Production Debugging

### 1. Conditional Logging
```rust
// Only in debug builds
#[cfg(debug_assertions)]
fn debug_log(&self, msg: &str) {
    println!("[DEBUG] {}", msg);
}

#[cfg(not(debug_assertions))]
fn debug_log(&self, _msg: &str) {
    // No-op in release
}
```

### 2. Error Reporting
```typescript
// Structured error reporting
export function reportError(error: Error, context: Record<string, any>) {
  const report = {
    message: error.message,
    stack: error.stack,
    context,
    timestamp: new Date().toISOString(),
    node: window.our?.node || 'unknown',
    userAgent: navigator.userAgent,
  };
  
  // In production, send to logging service
  if (import.meta.env.PROD) {
    // Send to your logging endpoint
    api.logError(report).catch(console.error);
  } else {
    console.error('Error Report:', report);
  }
}
```

## P2P Chat Testing Patterns (from samchat)

### Debug Logging for P2P Operations

```rust
// Node initialization
#[init]
async fn initialize(&mut self) {
    println!("Initializing Samchat state...");
    self.my_node_id = Some(our().node.clone());
    println!("Samchat initialized for node: {:?}", self.my_node_id);
}

// P2P message sending
async fn send_message_with_reply(&mut self, recipient: String, content: String, reply: Option<MessageReplyInfo>) -> Result<bool, String> {
    println!("send_message_with_reply called: to={}, content='{}', reply_to={:?}", recipient, content, reply);
    // ... implementation
}

// Remote message receipt
#[remote]
async fn receive_message(&mut self, message: ChatMessage) -> Result<bool, String> {
    println!("receive_message called: from={}, content='{}'", message.sender, message.content);
    
    // Track duplicates
    if !conversation.messages.iter().any(|m| m.id == message.id) {
        println!("Message {} received and persisted.", message.id);
    } else {
        println!("Duplicate message {} received, ignoring.", message.id);
    }
}

// Group operations
async fn create_group(&mut self, name: String, members: Vec<String>) -> Result<String, String> {
    println!("create_group called: name={}, members={:?}", name, members);
    // ... create group
    println!("Group created locally: {}", group_id);
    
    // Debug member notifications
    for participant in &participants {
        println!("Notifying {} about new group {}", participant, group_id);
    }
}
```

### Testing P2P Chat Scenarios

```bash
# P2P Chat Test Script
#!/bin/bash

# Start three nodes for group chat testing
kit s --fake-node alice.os &
ALICE_PID=$!
sleep 2

kit s --fake-node bob.os --port 8081 &
BOB_PID=$!
sleep 2

kit s --fake-node charlie.os --port 8082 &
CHARLIE_PID=$!
sleep 2

echo "Chat nodes started:"
echo "- Alice: http://localhost:8080"
echo "- Bob: http://localhost:8081"
echo "- Charlie: http://localhost:8082"
```

### P2P Chat Test Checklist

```typescript
export const CHAT_TEST_SCENARIOS = [
  {
    name: "Direct Message Delivery",
    steps: [
      "Open Alice and Bob nodes",
      "Send message from Alice to Bob",
      "Verify message appears on Bob's side",
      "Check sender info is correct",
      "Verify timestamps are consistent",
    ],
  },
  {
    name: "Group Chat Creation",
    steps: [
      "Create group on Alice with Bob and Charlie",
      "Verify group appears on all nodes",
      "Check member list is correct on all nodes",
      "Send message to group from Alice",
      "Verify all members receive the message",
      "Check sender attribution in group messages",
    ],
  },
  {
    name: "File Transfer Between Nodes",
    steps: [
      "Upload file on Alice node",
      "Send file message to Bob",
      "Verify file metadata appears correctly",
      "Download file on Bob's node",
      "Verify file content matches original",
      "Check local caching after download",
    ],
  },
  {
    name: "Message Reply Threading",
    steps: [
      "Send message from Alice to Bob",
      "Reply to message from Bob",
      "Verify reply context is preserved",
      "Check reply renders correctly on both sides",
      "Test reply in group conversations",
    ],
  },
  {
    name: "Offline Message Handling",
    steps: [
      "Send message from Alice to Bob",
      "Stop Bob's node",
      "Send another message from Alice",
      "Restart Bob's node",
      "Check if Bob receives pending messages",
      "Verify message order is preserved",
    ],
  },
];
```

### Debug Helpers for P2P Chat

```rust
// Debug endpoint to inspect conversations
#[http]
async fn debug_conversations(&self, _request_body: String) -> String {
    if cfg!(debug_assertions) {
        let debug_info = self.conversations.iter()
            .map(|(id, conv)| {
                json!({
                    "id": id,
                    "participants": conv.participants,
                    "message_count": conv.messages.len(),
                    "is_group": conv.is_group,
                    "group_name": conv.group_name,
                    "last_updated": conv.last_updated,
                })
            })
            .collect::<Vec<_>>();
        
        serde_json::to_string_pretty(&debug_info).unwrap()
    } else {
        "Debug disabled in production".to_string()
    }
}

// Check message delivery status
#[http]
async fn debug_check_delivery(&self, message_id: String) -> String {
    for (conv_id, conv) in &self.conversations {
        if let Some(msg) = conv.messages.iter().find(|m| m.id == message_id) {
            return json!({
                "found": true,
                "conversation": conv_id,
                "sender": msg.sender,
                "delivered": msg.delivered,
                "timestamp": msg.timestamp,
            }).to_string();
        }
    }
    json!({ "found": false }).to_string()
}
```

### Frontend Debug Panel for Chat

```typescript
// Debug panel for P2P chat
export const ChatDebugPanel: React.FC = () => {
  const { conversations, currentConversationMessages, myNodeId } = useSamchatStore();
  const [showDebug, setShowDebug] = useState(false);
  
  if (!import.meta.env.DEV) return null;
  
  return (
    <>
      <button 
        onClick={() => setShowDebug(!showDebug)}
        style={{ position: 'fixed', bottom: 10, left: 10, zIndex: 1000 }}
      >
        Debug
      </button>
      
      {showDebug && (
        <div style={{
          position: 'fixed',
          bottom: 50,
          left: 10,
          background: 'rgba(0,0,0,0.9)',
          color: 'white',
          padding: '10px',
          maxWidth: '400px',
          maxHeight: '300px',
          overflow: 'auto',
          fontSize: '12px',
          fontFamily: 'monospace',
        }}>
          <h4>Chat Debug Info</h4>
          <div>My Node: {myNodeId}</div>
          <div>Conversations: {conversations.length}</div>
          <div>Current Messages: {currentConversationMessages.length}</div>
          
          <h5>Conversation Details:</h5>
          {conversations.map(conv => (
            <div key={conv.id} style={{ marginBottom: '5px' }}>
              {conv.is_group ? '👥' : '💬'} {conv.group_name || conv.participants.join(' ↔ ')}
              <br />
              Last: {new Date(conv.last_updated).toLocaleTimeString()}
            </div>
          ))}
        </div>
      )}
    </>
  );
};
```

### Testing File Transfer Debug

```rust
// Track file operations
struct FileTransferDebug {
    uploads: Vec<(String, String, u64)>, // file_id, name, size
    downloads: Vec<(String, String, bool)>, // file_id, from_node, success
}

impl AppState {
    fn debug_file_transfer(&self, file_id: &str) {
        println!("=== FILE TRANSFER DEBUG ===");
        println!("File ID: {}", file_id);
        
        // Check local storage
        let local_path = format!("/samchat:hpn-testing-beta.os/files/{}", file_id);
        println!("Local path: {}", local_path);
        
        // Track transfer history
        if let Some(upload) = self.file_debug.uploads.iter().find(|(id, _, _)| id == file_id) {
            println!("Uploaded: {} ({} bytes)", upload.1, upload.2);
        }
        
        for download in &self.file_debug.downloads {
            if download.0 == file_id {
                println!("Downloaded from {}: {}", download.1, if download.2 { "✓" } else { "✗" });
            }
        }
        println!("========================");
    }
}
```

## Remember

1. **Always test P2P early** - Single node testing hides issues
2. **Log strategically** - Too much noise makes debugging harder
3. **Use proper error types** - Generic errors hide problems
4. **Test edge cases** - Network failures, concurrent updates
5. **Monitor performance** - Catch slowdowns before users do
6. **Document issues** - Future you will thank you
7. **Clean up debug code** - Don't ship console.logs to production
8. **Test group operations** - Multi-node scenarios reveal race conditions
9. **Debug message delivery** - Track messages across nodes
10. **Monitor file transfers** - P2P file sharing needs extra care