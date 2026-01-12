use crate::hyperware::process::tester::{
    FailResponse, Request as TesterRequest, Response as TesterResponse, RunRequest,
};
use hyperware_process_lib::{
    await_message, call_init, print_to_terminal, Address, ProcessId, Request, Response,
};
use serde_json::json;

mod tester_lib;

wit_bindgen::generate!({
    path: "../target/wit",
    world: "hyperapp-skeleton-test-template-dot-os-v0",
    generate_unused_types: true,
    additional_derives: [PartialEq, serde::Deserialize, serde::Serialize, process_macros::SerdeJsonInto],
});

call_init!(init);
fn init(our: Address) {
    print_to_terminal(0, "begin");

    loop {
        handle_message(&our);
    }
}

fn handle_message(our: &Address) {
    let message = await_message()
        .unwrap_or_else(|e| fail_with(format!("failed to receive tester message: {e:?}")));

    if !message.is_request() {
        fail_with("expected tester request message");
    }

    let source = message.source();
    if our.node != source.node {
        fail_with(format!("rejecting foreign message from {:?}", source));
    }

    let TesterRequest::Run(RunRequest {
        input_node_names: node_names,
        ..
    }) = message
        .body()
        .try_into()
        .unwrap_or_else(|e| fail_with(format!("failed to decode tester run request: {e:?}")));

    print_to_terminal(0, "hyperapp_skeleton_test: start");

    if our.node != node_names[0] {
        Response::new()
            .body(TesterResponse::Run(Ok(())))
            .send()
            .unwrap_or_else(|e| fail_with(format!("failed to send tester ack: {e:?}")));
        return;
    }

    let our_hyperapp_skeleton_address = Address {
        node: our.node.clone(),
        process: ProcessId::new(
            Some("hyperapp-skeleton"),
            "hyperapp-skeleton",
            "template.os",
        ),
    };

    increment_counter(&our_hyperapp_skeleton_address, 1);

    let local_messages = get_messages(&our_hyperapp_skeleton_address);
    if !local_messages
        .iter()
        .any(|m| m == "Counter incremented by 1")
    {
        fail_with(format!(
            "local hyperapp-skeleton did not store expected message: {local_messages:?}"
        ));
    }

    let status = get_status(&our_hyperapp_skeleton_address);
    let counter = status.counter;
    if counter != 1 {
        fail_with(format!(
            "local hyperapp-skeleton did not increment correctly: {counter:?}"
        ));
    }

    Response::new()
        .body(TesterResponse::Run(Ok(())))
        .send()
        .unwrap_or_else(|e| fail_with(format!("failed to send tester success: {e:?}")));
}

fn increment_counter(address: &Address, amount: u32) {
    let body = json!({"IncrementCounter": amount});
    let body = serde_json::to_vec(&body).unwrap();
    let request = Request::to(address).body(body);
    let response = request
        .send_and_await_response(15)
        .unwrap_or_else(|e| fail_with(format!("failed to send add_message request: {e:?}")))
        .unwrap_or_else(|_| fail_with("add_message returned no response"));

    if response.is_request() {
        fail!("increment_counter returned a request");
    }

    let result: Result<u32, String> = serde_json::from_slice(response.body())
        .unwrap_or_else(|e| fail_with(format!("failed to decode increment_counter response: {e}")));

    match result {
        Ok(counter) => {
            if counter == 0 {
                fail_with("increment_counter returned zero after increment");
            }
        }
        Err(err) => fail_with(format!("increment_counter returned error: {err}")),
    }
}

fn get_messages(address: &Address) -> Vec<String> {
    let payload = serde_json::to_vec(&json!({ "GetMessages": serde_json::Value::Null }))
        .unwrap_or_else(|e| fail_with(format!("failed to encode get_messages payload: {e}")));

    let response = Request::new()
        .target(address.clone())
        .body(payload)
        .send_and_await_response(15)
        .unwrap_or_else(|e| fail_with(format!("failed to send get_messages request: {e:?}")))
        .unwrap_or_else(|_| fail_with("get_messages returned no response"));

    if response.is_request() {
        fail_with("get_messages returned a request");
    }

    let result: Result<Vec<String>, String> = serde_json::from_slice(response.body())
        .unwrap_or_else(|e| fail_with(format!("failed to decode get_messages response: {e}")));

    match result {
        Ok(messages) => messages,
        Err(err) => fail_with(format!("get_messages returned error: {err}")),
    }
}

fn get_status(address: &Address) -> crate::hyperware::process::app::Status {
    let payload = serde_json::to_vec(&json!({ "GetStatus": serde_json::Value::Null }))
        .unwrap_or_else(|e| fail_with(format!("failed to encode get_status payload: {e}")));

    let response = Request::new()
        .target(address.clone())
        .body(payload)
        .send_and_await_response(15)
        .unwrap_or_else(|e| fail_with(format!("failed to send get_status request: {e:?}")))
        .unwrap_or_else(|_| fail_with("get_status returned no response"));

    if response.is_request() {
        fail_with("get_status returned a request");
    }

    let result: Result<crate::hyperware::process::app::Status, String> =
        serde_json::from_slice(response.body())
            .unwrap_or_else(|e| fail_with(format!("failed to decode get_status response: {e}")));

    match result {
        Ok(status) => status,
        Err(err) => fail_with(format!("get_status returned error: {err}")),
    }
}

fn fail_with(message: impl Into<String>) -> ! {
    let message = message.into();
    let log = format!("hyperapp_skeleton_test: error: {message}");
    print_to_terminal(0, log.as_str());
    fail!(message);
}
