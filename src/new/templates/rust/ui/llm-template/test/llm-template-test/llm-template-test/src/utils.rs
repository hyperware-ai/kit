use crate::*;

pub fn create_log_file() -> anyhow::Result<File> {
    let our = our();
    kiprintln!("Creating log file for {}", our);
    
    let drive_path = match create_drive(our.package_id(), "test_logs", Some(5)) {
        Ok(path) => {
            kiprintln!("Created drive at path: {}", path);
            path
        },
        Err(e) => {
            kiprintln!("Failed to create drive: {:?}", e);
            return Err(e.into());
        }
    };
    
    let file_path = format!("{}/llm-template-test.log", drive_path);
    kiprintln!("Creating log file at: {}", file_path);
    
    match create_file(&file_path, None) {
        Ok(file) => {
            kiprintln!("Log file created successfully");
            Ok(file)
        },
        Err(e) => {
            kiprintln!("Failed to create log file: {:?}", e);
            Err(e.into())
        }
    }
}

pub fn write_log(file: &mut File, msg: &str) -> anyhow::Result<()> {
    file.write_all(format!("{}\n", msg).as_bytes())?;
    file.sync_all()?;
    Ok(())
}

pub fn get_client_addresses(node_names: &Vec<String>) -> anyhow::Result<Vec<Address>> {
    kiprintln!("Processing node names: {:?}", node_names);
    
    let client_addresses: Vec<Address> = node_names
        .iter()
        .filter(|name| {
            // This will match "client0.os", "client1.os", etc.
            let is_client = name.contains("client");
            kiprintln!("Node {} is{} a client", name, if is_client {""} else {" not"});
            is_client
        })
        .map(|name| {
            let address: Address = (name, "llm-template", "llm-template", "template.os").into();
            address
        })
        .collect();
    
    kiprintln!("Found {} client addresses: {:?}", client_addresses.len(), client_addresses);
    
    if client_addresses.is_empty() {
        return Err(anyhow::anyhow!("No client addresses found"));
    }
    
    Ok(client_addresses)
}



