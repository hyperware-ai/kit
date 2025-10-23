// Type definitions for the Hyperapp Skeleton App
// These should match the types defined in your Rust backend

// Store state interface
export interface HyperappSkeletonState {
  // Connection state
  nodeId: string | null;
  isConnected: boolean;
  
  // App data (mirrors backend state)
  counter: number;
  messages: string[];
  
  // UI state
  isLoading: boolean;
  error: string | null;
}
