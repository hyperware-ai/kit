use crate::*;
use hyperware_process_lib::{Address, Request};
use serde_json::to_vec;
use crate::hyperware::process::llm_template::{HyperApiRequest, HyperApiResponse, CustomMessage};

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
    let get_status_request = HyperApiRequest::GetStatus;
    let get_history_request = HyperApiRequest::GetHistory;
    let message_request = HyperApiRequest::Message(CustomMessage { 
        message_type: "test".to_string(), 
        content: "test message".to_string() 
    });

    // Send GetStatus request
    let status_request_bytes = to_vec(&get_status_request).unwrap();
    let status_response = Request::to(client.clone())
        .body(status_request_bytes)
        .send_and_await_response(10)??
        .body()
        .to_vec();
    match serde_json::from_slice::<HyperApiResponse>(&status_response) {
        Ok(response) => write_log(log_file, &format!("GetStatus response from client {}: {:?}", client, response))?,
        Err(e) => {
            write_log(log_file, &format!("GetStatus error parsing response from client {}: {:?}", client, e))?;
            write_log(log_file, &format!("Raw response: {}", String::from_utf8_lossy(&status_response)))?;
        }
    }

    // Send GetHistory request
    let history_request_bytes = to_vec(&get_history_request).unwrap();
    let history_response = Request::to(client.clone())
        .body(history_request_bytes)
        .send_and_await_response(10)??
        .body()
        .to_vec();
    match serde_json::from_slice::<HyperApiResponse>(&history_response) {
        Ok(response) => write_log(log_file, &format!("GetHistory response from client {}: {:?}", client, response))?,
        Err(e) => {
            write_log(log_file, &format!("GetHistory error parsing response from client {}: {:?}", client, e))?;
            write_log(log_file, &format!("Raw response: {}", String::from_utf8_lossy(&history_response)))?;
        }
    }

    // Send Message request
    let message_request_bytes = to_vec(&message_request).unwrap();
    let message_response = Request::to(client.clone())
        .body(message_request_bytes)
        .send_and_await_response(10)??
        .body()
        .to_vec();
    match serde_json::from_slice::<HyperApiResponse>(&message_response) {
        Ok(response) => write_log(log_file, &format!("Message response from client {}: {:?}", client, response))?,
        Err(e) => {
            write_log(log_file, &format!("Message error parsing response from client {}: {:?}", client, e))?;
            write_log(log_file, &format!("Raw response: {}", String::from_utf8_lossy(&message_response)))?;
        }
    }

    write_log(log_file, &format!("All operations completed for client {}", client))?;
    
    Ok(())
}