use crate::*;
use hyperware_process_lib::{Address, Request};
use serde_json::to_vec;
use shared_types::ApiRequest;

pub fn run_client_ops(log_file: &mut File, client_addresses: &Vec<Address>) -> anyhow::Result<()> {
    for client in client_addresses.iter() {
        send_client_operation(client, log_file)?;
        write_log(
            log_file,
            &format!(
                "Done running client operations for {}, ", client, 
            ),
        )?;
    }
    write_log(log_file, &format!("Done creating curations"))?;
    Ok(())
}

fn send_client_operation(client: &Address, log_file: &mut File) -> anyhow::Result<()> {
    let get_status_request = ApiRequest::GetStatus;
    let get_history_request = ApiRequest::GetHistory;
    let custom_message_request = ApiRequest::CustomMessage { 
        message_type: "test".to_string(), 
        content: "test message".to_string() 
    };

    // Send GetStatus request
    let status_request_bytes = to_vec(&get_status_request).unwrap();
    let status_response = Request::to(client.clone())
        .body(status_request_bytes)
        .send_and_await_response(10)??
        .body()
        .to_vec();
    match serde_json::from_slice::<serde_json::Value>(&status_response) {
        Ok(value) => write_log(log_file, &format!("GetStatus response from client {}: {:?}", client, value))?,
        Err(e) => write_log(log_file, &format!("GetStatus error parsing response from client {}: {:?}", client, e))?,
    }

    // Send GetHistory request
    let history_request_bytes = to_vec(&get_history_request).unwrap();
    let history_response = Request::to(client.clone())
        .body(history_request_bytes)
        .send_and_await_response(10)??
        .body()
        .to_vec();
    match serde_json::from_slice::<serde_json::Value>(&history_response) {
        Ok(value) => write_log(log_file, &format!("GetHistory response from client {}: {:?}", client, value))?,
        Err(e) => write_log(log_file, &format!("GetHistory error parsing response from client {}: {:?}", client, e))?,
    }

    // Send CustomMessage request
    let custom_request_bytes = to_vec(&custom_message_request).unwrap();
    let custom_response = Request::to(client.clone())
        .body(custom_request_bytes)
        .send_and_await_response(10)??
        .body()
        .to_vec();
    match serde_json::from_slice::<serde_json::Value>(&custom_response) {
        Ok(value) => write_log(log_file, &format!("CustomMessage response from client {}: {:?}", client, value))?,
        Err(e) => write_log(log_file, &format!("CustomMessage error parsing response from client {}: {:?}", client, e))?,
    }

    write_log(log_file, &format!("All operations completed for client {}", client))?;
    
    Ok(())
}