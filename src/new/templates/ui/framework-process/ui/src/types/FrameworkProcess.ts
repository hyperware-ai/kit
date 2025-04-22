// Define the type for the state managed by the Zustand store
export interface FrameworkProcessState {
  items: string[];
}

// --- Request Types ---

// Request body for the submit_entry endpoint
export interface SubmitEntryRequest {
    SubmitEntry: string; // Key matches the Rust function name
}

// Request body for the view_state endpoint 
// Takes an empty string argument as per the Rust backend
export interface ViewStateRequest {
    ViewState: string; // Key matches the Rust function name
}

// --- Response Types ---

// Response type for the submit_entry endpoint
// Wraps the boolean result in 'Ok'
export interface SubmitEntryResponse {
  Ok: boolean;
  // Err?: string; // Optional: if the backend sends specific error shapes
} 

// Response type for the view_state endpoint
// Wraps the string array result in 'Ok'
export interface ViewStateResponse {
  Ok: string[];
  // Err?: string; // Optional: if the backend sends specific error shapes
}

export interface SubmitEntry {
    SubmitEntry: string;
}

export interface ViewState {
    ViewState: string;
}

