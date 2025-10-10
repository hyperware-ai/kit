// Define the structure for a single todo item
export interface TodoItem {
  id: string;
  text: string;
  completed: boolean;
}

// Define the type for the state managed by the Zustand store
export interface HyperappTodoState {
  tasks: TodoItem[]; // State now holds an array of TodoItems
}

// --- Request Types ---

// Request body for the add_task endpoint
export interface AddTaskRequest {
    AddTask: string; // Key matches the Rust function name, value is the task text
}

// Request body for the get_tasks endpoint
// Takes an empty string argument as per the Rust backend requirement for POST
export interface GetTasksRequest {
    GetTasks: string; // Key matches the Rust function name, value is empty string
}

// Request body for the toggle_task endpoint
export interface ToggleTaskRequest {
    ToggleTask: string; // Key matches the Rust function name, value is the task ID
}


// --- Response Types ---
// Generic response wrapper for Rust Result<T, E> where E is String
interface RustResponse<T> {
  Ok?: T;
  Err?: string;
}

// Response type for the add_task endpoint
export type AddTaskResponse = RustResponse<TodoItem>;

// Response type for the get_tasks endpoint
export type GetTasksResponse = RustResponse<TodoItem[]>;

// Response type for the toggle_task endpoint
export type ToggleTaskResponse = RustResponse<TodoItem>;


// --- Remove Old Types ---
// export interface SubmitEntryRequest { ... }
// export interface ViewStateRequest { ... }
// export interface SubmitEntryResponse { ... }
// export interface ViewStateResponse { ... }
// export interface SubmitEntry { ... }
// export interface ViewState { ... }

