use caller_utils::{HyperappTodoState, TodoItem};
use caller_utils::hyperapp_todo::{export_state_local_rpc, import_state_local_rpc};
// Add this import here, as fail! is expanded in this file
use crate::hyperware::process::tester::{FailResponse, Response as TesterResponse};

use serde_json; 
mod tester_lib;


async_test_suite!(
    "hyperapp-todo-test-template-dot-os-v0",

    test_basic_math: async {
        if 2 + 2 != 4 {
            fail!("wrong result");
        }
        Ok(())
    },

    // Test importing and exporting state locally
    test_import_export_state: async {
        let address: Address = ("hyperapp-todo.os", "hyperapp-todo", "hyperapp-todo", "template.os").into();

        // 1. Define initial state (dummy data)
        let initial_state = HyperappTodoState {
            tasks: vec![
                TodoItem { id: "1".to_string(), text: "Task 1".to_string(), completed: false },
                TodoItem { id: "2".to_string(), text: "Task 2".to_string(), completed: true },
            ],
        };
        print_to_terminal(0, &format!("Initial state: {:?}", initial_state));


        // 2. Serialize the initial state for import
        let import_data = serde_json::to_vec(&initial_state)
            .map_err(|e| anyhow::anyhow!(format!("Failed to serialize initial state: {}", e)))?;
        print_to_terminal(0, "Serialized initial state for import.");

        // 3. Call import_state_local_rpc
        let import_result = import_state_local_rpc(&address, import_data).await;
        print_to_terminal(0, &format!("import_state_local_rpc result: {:?}", import_result));

        // Assert import was successful
        match import_result {
            Ok(Ok(true)) => print_to_terminal(0, "Import successful (returned true)."),
            Ok(Ok(false)) => {
                fail!("import_state_local_rpc returned false");
            }
            Ok(Err(e)) => {
                fail!(format!("import_state_local_rpc returned an error: {}", e));
            }
            Err(e) => {
                fail!(format!("import_state_local_rpc failed (send error): {:?}", e));
            }
        }

        // 4. Call export_state_local_rpc
        let export_result = export_state_local_rpc(&address).await;
        print_to_terminal(0, &format!("export_state_local_rpc result: {:?}", export_result));

        // Assert export was successful and get data, handling errors first
        let inner_result = match export_result {
            Ok(res) => res,
            Err(e) => {
                fail!(format!("export_state_local_rpc failed (send error): {:?}", e));
            }
        };
        let exported_data = match inner_result {
            Ok(data) => data,
            Err(e) => {
                fail!(format!("export_state_local_rpc returned an error: {}", e));
            }
        };
        print_to_terminal(0, "Exported state data received.");

        // 5. Compare initial state with exported state manually 
        if initial_state.tasks.len() != exported_data.tasks.len() {
            fail!(format!(
                "Task list lengths differ. Expected: {}, Got: {}",
                initial_state.tasks.len(),
                exported_data.tasks.len()
            ));
        }

        // Iterate and compare each task field by field
        for (initial_task, exported_task) in initial_state.tasks.iter().zip(exported_data.tasks.iter()) {
            if initial_task.id != exported_task.id ||
               initial_task.text != exported_task.text ||
               initial_task.completed != exported_task.completed {
                fail!(format!(
                    "Task mismatch detected.\nExpected Task: {:?}\nGot Task: {:?}",
                    initial_task, // Assumes TodoItem derives Debug
                    exported_task  // Assumes TodoItem derives Debug
                ));
            }
        }
        print_to_terminal(0, "Exported state matches initial state.");

        Ok(())
    },

);