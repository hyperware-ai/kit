// Import necessary types and functions explicitly from caller_utils
use caller_utils::{Argument, ReturnValue}; // Import types directly (via path dependency)
use caller_utils::hyperapp_echo::{echo_local_rpc, echo_remote_rpc}; // Import RPC functions

// Add this import here, as fail! is expanded in this file
use crate::hyperware::process::tester::{FailResponse, Response as TesterResponse};

mod tester_lib;

async_test_suite!(
    "hyperapp-echo-test-template-dot-os-v0",

    test_basic_math: async {
        if 2 + 2 != 4 {
            fail!("wrong result");
        }
        Ok(())
    },

    // Test local echo RPC call
    test_echo_local_rpc: async {
        // Define the target process address
        let address: Address = ("hyperapp-echo.os", "hyperapp-echo", "hyperapp-echo", "template.os").into();
        // Define the argument for the echo function
        let arg = Argument {
            header: "LocalTestHeader".to_string(),
            body: "LocalTestBody".to_string(),
        };
        // Define the expected return value
        let expected_return = ReturnValue {
            response: "Ack".to_string(),
        };

        match echo_local_rpc(&address, arg).await {
            Ok(actual_value) => {
                // Compare the 'response' field directly
                if actual_value.response != expected_return.response {
                    // fail! macro uses FailResponse/TesterResponse imported above
                    fail!(format!(
                        "echo_local_rpc unexpected result: expected {:?}, got {:?}",
                        expected_return, actual_value // Keep original structs for error message
                    ));
                }
                // If the result matches, the test passes for this step
                Ok(())
            }
            Err(e) => {
                // Use fail! macro if the RPC call itself returned an error
                fail!(format!("echo_local_rpc failed: {:?}", e));
            }
        }
    },

    // Test remote echo RPC call
    test_echo_remote_rpc: async {
        // Define the target process address
        let address: Address = ("hyperapp-echo.os", "hyperapp-echo", "hyperapp-echo", "template.os").into();
        // Define the argument for the echo function
        let arg = Argument {
            header: "RemoteTestHeader".to_string(),
            body: "RemoteTestBody".to_string(),
        };
        // Define the expected return value
        let expected_return = ReturnValue {
            response: "Ack".to_string(),
        };

        // Call the remote echo RPC stub
        match echo_remote_rpc(&address, arg).await {
            Ok(actual_value) => {
                // Compare the 'response' field directly
                if actual_value.response != expected_return.response {
                    // fail! macro uses FailResponse/TesterResponse imported above
                    fail!(format!(
                        "echo_remote_rpc unexpected result: expected {:?}, got {:?}",
                        expected_return, actual_value // Keep original structs for error message
                    ));
                }
                // If the result matches, the test passes for this step
                Ok(())
            }
            Err(e) => {
                 // Use fail! macro if the RPC call itself returned an error
                 fail!(format!("echo_remote_rpc failed: {:?}", e));
            }
        }
    },
);