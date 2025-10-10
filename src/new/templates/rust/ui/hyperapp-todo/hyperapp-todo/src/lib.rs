use hyperprocess_macro::hyperprocess;
use serde::{Deserialize, Serialize};
use uuid::Uuid; // Keep Uuid

// --- Todo Item ---
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct TodoItem {
    id: String,
    text: String,
    completed: bool,
}

// --- State ---
// Add Clone for potential use in export/internal logic if needed.
#[derive(PartialEq, Clone, Default, Debug, Serialize, Deserialize)]
pub struct HyperappTodoState {
    tasks: Vec<TodoItem>,
}

// --- Hyperware Process ---
#[hyperprocess(
    name = "hyperapp-todo",
    ui = Some(HttpBindingConfig::default()),
    endpoints = vec![
        Binding::Http {
            path: "/api",
            config: HttpBindingConfig::new(false, false, false, None),
        },
        Binding::Ws {
            path: "/ws",
            config: WsBindingConfig::new(false, false, false),
        }
    ],
    save_config = SaveOptions::EveryMessage,
    wit_world = "hyperapp-todo-template-dot-os-v0"
)]

// --- Hyperware Process API definitions ---
impl HyperappTodoState {
    #[init]
    async fn initialize(&mut self) {
        println!("Initializing todo list state");
        self.tasks = Vec::new();
    }

    // Add a new task
    #[http]
    async fn add_task(&mut self, text: String) -> Result<TodoItem, String> {
        if text.trim().is_empty() {
            return Err("Task text cannot be empty".to_string());
        }
        let new_task = TodoItem {
            id: Uuid::new_v4().to_string(),
            text,
            completed: false,
        };
        self.tasks.push(new_task.clone());
        println!("Added task: {:?}", new_task);
        Ok(new_task)
    }

    // Get all tasks
    #[http]
    async fn get_tasks(&self, request: String) -> Result<Vec<TodoItem>, String> {
        println!("Request: {:?}", request);
        println!("Fetching tasks");
        Ok(self.tasks.clone())
    }


    // Toggle completion status of a task
    #[http]
    async fn toggle_task(&mut self, id: String) -> Result<TodoItem, String> {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.completed = !task.completed;
            println!("Toggled task {:?}: completed={}", task.id, task.completed);
            Ok(task.clone())
        } else {
            Err(format!("Task with ID {} not found", id))
        }
    }

    // Export the current state (all tasks) as JSON bytes
    #[local]
    #[remote]
    async fn export_state(&self) -> Result<HyperappTodoState, String> {
        println!("Exporting tasks request received");
        // Return the state directly instead of serializing it
        Ok(self.clone())
    }

    // Import tasks from JSON bytes, replacing the current tasks
    #[local]
    async fn import_state(&mut self, data: Vec<u8>) -> Result<bool, String> {
        println!("Importing tasks request received");
        // Deserialize the data into the state struct using from_slice for Vec<u8>
        let imported_state: HyperappTodoState = serde_json::from_slice(&data)
            .map_err(|e| format!("Failed to deserialize state data: {}", e))?;
        // Replace the current tasks with the imported ones
        self.tasks = imported_state.tasks;
        println!("Tasks imported successfully");
        Ok(true)
    }

}
