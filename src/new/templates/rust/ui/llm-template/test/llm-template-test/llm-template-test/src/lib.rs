use crate::hyperware::process::tester::{Request as TesterRequest, Response as TesterResponse, RunRequest, FailResponse};
use hyperware_process_lib::{await_message, call_init, print_to_terminal, println, Address, ProcessId, Request, Response, kiprintln,
    http::server::{
        send_response, HttpServer, HttpServerRequest, StatusCode, send_ws_push, WsMessageType,
    },
    vfs::{create_drive, create_file, File},
    our,
};
mod utils;
mod client_ops;
mod tester_lib;

use utils::*;
use client_ops::*;

// Add type alias to disambiguate Error
type ConversionError = core::convert::Infallible;

wit_bindgen::generate!({
    path: "target/wit",
    world: "llm-template-test-template-dot-os-v0",
    generate_unused_types: true,
    additional_derives: [PartialEq, serde::Deserialize, serde::Serialize, process_macros::SerdeJsonInto],
});
fn handle_message(log_file: &mut File) -> anyhow::Result<()> {
    kiprintln!("handle_message called");
    
    match run_tests(log_file) {
        Ok(_) => {
            kiprintln!("Tests completed successfully");
            write_log(log_file, "Tests completed successfully")?;
            Response::new()
                .body(TesterResponse::Run(Ok(())))
                .send()
                .unwrap();
            kiprintln!("Sent success response");
        },
        Err(e) => {
            kiprintln!("Error running tests: {:?}", e);
        }
    }

    kiprintln!("handle_message completed");
    Ok(())
}
fn init_tests(our: Address) -> anyhow::Result<Vec<String>> {
    kiprintln!("Init tests called with our address: {}", our);
    
    let message = match await_message() {
        Ok(msg) => msg,
        Err(e) => {
            kiprintln!("Error awaiting message: {:?}", e);
            return Err(anyhow::anyhow!("Error awaiting message: {:?}", e));
        }
    };
    
    if !message.is_request() {
        kiprintln!("Received message is not a request");
        fail!("received-non-request");
        // The fail! macro will panic, so this won't be reached
    }

    let source = message.source();
    kiprintln!("Message source: {:?}", source);
    
    if our.node != source.node {
        kiprintln!("Rejecting foreign message from {:?}", source);
        return Err(anyhow::anyhow!(
            "rejecting foreign Message from {:?}",
            source,
        ));
    }

    let request = match message.body().try_into() {
        Ok(TesterRequest::Run(req)) => req,
        Err(e) => {
            fail!("error-parsing-message-body");
            kiprintln!("Error parsing message body: {:?}", e);
            return Err(anyhow::anyhow!("Error parsing message body: {:?}", e));
        }
    };
    
    let node_names = request.input_node_names;
    kiprintln!("node_names: {:?}", node_names);
    
    if node_names.len() < 2 {
        fail!("not-enough-nodes");
    }
    
    if our.node != node_names[0] {
        kiprintln!("We are not the master node. Our: {}, master: {}", our.node, node_names[0]);
        // we are not master node: return
        Response::new()
            .body(TesterResponse::Run(Ok(())))
            .send()
            .unwrap();
        return Ok(vec![]);
    }
    
    kiprintln!("We are the master node, proceeding with test");
    Ok(node_names)
}

fn run_tests(log_file: &mut File) -> anyhow::Result<()> {
    let client_node_names = init_tests(our())?;
    write_log(log_file, &format!("Found client nodes: {:?}", client_node_names))?;
    
    if client_node_names.is_empty() {
        write_log(log_file, "No client nodes to test, exiting early")?;
        return Ok(());
    }
    
    let client_addresses = get_client_addresses(&client_node_names)?;
    write_log(log_file, &format!("Client addresses: {:?}", client_addresses))?;

    write_log(log_file, "----------------------------------------")?;
    write_log(log_file, "Starting client operations")?;
    run_client_ops(log_file, &client_addresses)?;
    write_log(log_file, "----------------------------------------")?;
    write_log(log_file, "Done running client operations")?;

    Ok(())
}

call_init!(init);
fn init(_our: Address) -> anyhow::Result<()> {
    let mut log_file = create_log_file()?;

    loop {
        match handle_message(&mut log_file) {
            Ok(()) => {},
            Err(e) => {
                print_to_terminal(0, format!("sample_test_app_test: error: {e:?}").as_str());
                fail!("sample_test_app_test");
            },
        };
    }
}
