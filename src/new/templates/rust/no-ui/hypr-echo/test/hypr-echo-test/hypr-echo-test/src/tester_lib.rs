#[allow(unused_imports)]
use crate::hyperware::process::tester::{FailResponse, Response as TesterResponse};
use hyperware_process_lib::Response;
use hyperware_app_common::SendResult;

#[macro_export]
macro_rules! fail {
    ($test:expr) => {
        Response::new()
            .body(TesterResponse::Run(Err(FailResponse {
                test: $test.into(),
                file: file!().into(),
                line: line!(),
                column: column!(),
            })))
            .send()
            .unwrap();
        panic!("")
    };
    ($test:expr, $file:expr, $line:expr, $column:expr) => {
        Response::new()
            .body(TesterResponse::Run(Err(FailResponse {
                test: $test.into(),
                file: $file.into(),
                line: $line,
                column: $column,
            })))
            .send()
            .unwrap();
        panic!("")
    };
}

#[macro_export]
macro_rules! async_test_suite {
    ($wit_world:expr, $($test_name:ident: async $test_body:block),* $(,)?) => {
        wit_bindgen::generate!({
            path: "target/wit",
            world: $wit_world,
            generate_unused_types: true,
            additional_derives: [PartialEq, serde::Deserialize, serde::Serialize, process_macros::SerdeJsonInto],
        });

        use hyperware_process_lib::{
            await_message, call_init, print_to_terminal, Address, Response
        };
        
        $(
            async fn $test_name() -> anyhow::Result<()> {
                $test_body
            }
        )*
        
        async fn run_all_tests() -> anyhow::Result<()> {
            $(
                print_to_terminal(0, concat!("Running test: ", stringify!($test_name)));
                match $test_name().await {
                    Ok(()) => {
                        print_to_terminal(0, concat!("Test passed: ", stringify!($test_name)));
                    },
                    Err(e) => {
                        print_to_terminal(0, &format!("Test failed: {} - {:?}", stringify!($test_name), e));
                        return Err(e);
                    }
                }
            )*
            
            print_to_terminal(0, "All tests passed!");
            Ok(())
        }

        call_init!(init);
        fn init(_our: Address) {
            print_to_terminal(0, "Starting test suite...");
            
            // Flag to track if tests have been triggered and started
            let mut tests_triggered = false;
            
            // Main event loop
            loop {
                // Poll tasks to advance the executor
                hyperware_app_common::APP_CONTEXT.with(|ctx| {
                    ctx.borrow_mut().executor.poll_all_tasks();
                });
                
                // First, process any messages to handle RPC responses
                match await_message() {
                    Ok(message) => {
                        match message {
                            hyperware_process_lib::Message::Response {body, context, ..} => {
                                // Handle responses to unblock waiting futures
                                let correlation_id = context
                                    .as_deref()
                                    .map(|bytes| String::from_utf8_lossy(bytes).to_string())
                                    .unwrap_or_else(|| "no context".to_string());
                                
                                print_to_terminal(0, &format!("Received response with ID: {}", correlation_id));
                                
                                hyperware_app_common::RESPONSE_REGISTRY.with(|registry| {
                                    let mut registry_mut = registry.borrow_mut();
                                    registry_mut.insert(correlation_id, body);
                                });
                            },
                            hyperware_process_lib::Message::Request { .. } => {
                                // The first request triggers test execution
                                if !tests_triggered {
                                    tests_triggered = true;
                                    print_to_terminal(0, "Received initial request, starting tests...");
                                    
                                    // Start the test suite
                                    hyperware_app_common::hyper! {
                                        match run_all_tests().await {
                                            Ok(()) => {
                                                // All tests passed - send success response
                                                print_to_terminal(0, "Tests completed successfully!");
                                                Response::new()
                                                    .body(TesterResponse::Run(Ok(())))
                                                    .send()
                                                    .unwrap_or_else(|e| {
                                                        print_to_terminal(0, &format!("Failed to send success response: {:?}", e));
                                                    });
                                            },
                                            Err(e) => {
                                                // Tests failed - send failure response
                                                print_to_terminal(0, &format!("Test suite failed: {:?}", e));
                                                crate::fail!(&format!("Test failure: {:?}", e));
                                            }
                                        }
                                    }
                                }
                                // No response here - response is sent when tests complete
                            }
                        }
                    },
                    Err(e) => {
                        // Handle send errors to unblock futures that are waiting for responses
                        if let hyperware_process_lib::SendError {
                            kind,
                            context: Some(context),
                            ..
                        } = &e
                        {
                            if let Ok(correlation_id) = String::from_utf8(context.to_vec()) {
                                let error_response = serde_json::to_vec(kind).unwrap();
                                
                                hyperware_app_common::RESPONSE_REGISTRY.with(|registry| {
                                    let mut registry_mut = registry.borrow_mut();
                                    registry_mut.insert(correlation_id, error_response);
                                });
                            }
                        }
                        
                        print_to_terminal(0, &format!("Message error: {:?}", e));
                    }
                }
            }
        }
    };
}

/// Helper function to test remote RPC calls
/// 
/// This function handles:
/// 1. Checking if the call was successful
/// 2. Validating the returned value against an expected value
/// 3. Handling error cases with appropriate failure messages
/// 
/// Returns the actual value if successful, allowing it to be used in subsequent operations
pub async fn test_remote_call<T, F>(
    call_future: F,
    expected_value: T,
    error_msg: &str,
) -> anyhow::Result<T>
where
    T: std::cmp::PartialEq + std::fmt::Debug + Clone,
    F: std::future::Future<Output = SendResult<T>>,
{
    let result = call_future.await;
    
    match result {
        SendResult::Success(actual) => {
            if actual != expected_value {
                fail!(format!("{}: expected {:?}, got {:?}", error_msg, expected_value, actual));
            }
            // Return the actual value
            Ok(actual)
        }
        _ => {
            fail!(match result {
                SendResult::Timeout => "timeout",
                SendResult::Offline => "offline",
                SendResult::DeserializationError(_) => "deserialization error",
                _ => "unknown error",
            });
        }
    }
}