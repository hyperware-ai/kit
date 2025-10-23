use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use color_eyre::{
    eyre::{eyre, Result},
    Section,
};
use reqwest::Client;
use serde::Deserialize;
use tokio::time::{sleep, Duration};
use tracing::{debug, info, instrument};

use crate::build;
use crate::run_tests::cleanup::{clean_process_by_pid, cleanup_on_signal};
use crate::run_tests::types::BroadcastRecvBool;
use crate::setup::{check_foundry_deps, get_deps};
use crate::KIT_CACHE;

// First account on anvil
const OWNER_ADDRESS: &str = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266";

const DEFAULT_MAX_ATTEMPTS: u16 = 16;
const DEFAULT_CONFIG_PATH: &str = "./Contracts.toml";

#[derive(Debug, Clone)]
struct ContractAddresses {
    hypermap_proxy: String,
    hypermap_impl: String,
    hyperaccount: String,
    erc6551registry: String,
    multicall: String,
    create2: String,
    dot_os_tba: String,
    zeroth_tba: String,
}

impl ContractAddresses {
    async fn from_config(
        port: u16,
        config: &ChainConfig,
        deployed: &HashMap<String, String>,
    ) -> Result<Self> {
        let resolve = |name: &str| -> Result<String> {
            // First try deployed addresses
            if let Some(addr) = deployed.get(name) {
                return Ok(addr.clone());
            }
            // Then try config
            if let Some(addr) = config.get_address_by_name(name) {
                return Ok(addr);
            }
            Err(eyre!("Missing '{}' in config and deployed contracts", name))
        };

        let resolve_optional = |name: &str, default: &str| -> String {
            deployed
                .get(name)
                .cloned()
                .or_else(|| config.get_address_by_name(name))
                .unwrap_or_else(|| default.to_string())
        };

        let hypermap_proxy = resolve("hypermap-proxy")?;

        // Call tbaOf(0) to get zeroth_tba address
        // cast calldata "tbaOf(uint256)" 0
        let tba_of_zero_calldata =
            "0x27244d1e0000000000000000000000000000000000000000000000000000000000000000";
        let zeroth_tba_result = call_contract(port, &hypermap_proxy, tba_of_zero_calldata).await?;

        // Extract address from result (last 20 bytes / 40 hex chars)
        let zeroth_tba = if zeroth_tba_result.len() >= 42 {
            format!("0x{}", &zeroth_tba_result[zeroth_tba_result.len() - 40..])
        } else {
            resolve_optional("zeroth-tba", "0x809A598d9883f2Fb6B77382eBfC9473Fd6A857c9")
        };

        info!("Resolved zeroth_tba from hypermap: {}", zeroth_tba);

        Ok(Self {
            hypermap_proxy,
            hypermap_impl: resolve("hypermap-impl")?,
            hyperaccount: resolve("hyperaccount")?,
            erc6551registry: resolve("erc6551registry")?,
            multicall: resolve("multicall")?,
            create2: resolve("create2")?,
            dot_os_tba: resolve_optional(
                "dot-os-tba",
                "0x9b3853358ede717fc7D4806cF75d7A4d4517A9C9",
            ),
            zeroth_tba,
        })
    }

    fn print_summary(&self) {
        info!("╔════════════════════════════════════════════════════════════════╗");
        info!("║              Contract Addresses Summary                        ║");
        info!("╠════════════════════════════════════════════════════════════════╣");
        info!("║ hypermap_proxy:   {}   ║", self.hypermap_proxy);
        info!("║ hypermap_impl:    {}   ║", self.hypermap_impl);
        info!("║ hyperaccount:     {}   ║", self.hyperaccount);
        info!("║ erc6551registry:  {}   ║", self.erc6551registry);
        info!("║ multicall:        {}   ║", self.multicall);
        info!("║ create2:          {}   ║", self.create2);
        info!("║ dot_os_tba:       {}   ║", self.dot_os_tba);
        info!("║ zeroth_tba:       {}   ║", self.zeroth_tba);
        info!("╚════════════════════════════════════════════════════════════════╝");
    }

    fn get_transactions(&self) -> Vec<(String, String)> {
        vec![
            // Initialize Hypermap: give ownership to OWNER_ADDRESS
            (
                self.hypermap_proxy.clone(),
                "0xc4d66de8000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266"
                    .to_string(),
            ),
            // CREATE2 deploy HyperAccountMinter
            (
                self.create2.clone(),
                include_str!("./bytecode/deploy-hyperaccount-minter.txt").to_string(),
            ),
            // CREATE2 deploy HyperAccountPermissionedMinter
            (
                self.create2.clone(),
                include_str!("./bytecode/deploy-hyperaccount-permissioned-minter.txt").to_string(),
            ),
            // CREATE2 deploy HyperAccount9CharCommitMinter
            (
                self.create2.clone(),
                include_str!("./bytecode/deploy-hyperaccount-9char-commit-minter.txt").to_string(),
            ),
            // Mint .os
            (
                self.zeroth_tba.clone(),
                include_str!("./bytecode/mint-os.txt").to_string(),
            ),
        ]
    }
}

#[derive(Debug, Deserialize)]
struct ChainConfig {
    #[serde(default)]
    contracts: Vec<ContractConfig>,

    #[serde(default)]
    transactions: Vec<TransactionConfig>,
}

#[derive(Debug, Deserialize)]
struct ContractConfig {
    #[serde(default)]
    name: Option<String>,

    // For deployment (address will be computed)
    #[serde(default)]
    contract_json_path: Option<String>,

    // For setting code at known address
    #[serde(default)]
    address: Option<String>,

    #[serde(default)]
    bytecode: Option<String>,

    #[serde(default)]
    deployed_bytecode_path: Option<String>,

    #[serde(default)]
    constructor_args: Vec<ConstructorArg>,

    #[serde(default)]
    storage: HashMap<String, StorageValue>,
}

#[derive(Debug, Deserialize, Clone)]
struct TransactionConfig {
    #[serde(default)]
    name: Option<String>,

    target: String,

    #[serde(default)]
    function_signature: Option<String>,

    #[serde(default)]
    args: Vec<ConstructorArg>,

    #[serde(default)]
    data: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct ConstructorArg {
    #[serde(rename = "type")]
    arg_type: String,
    value: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
enum StorageValue {
    String(String),
    Number(u64),
}

impl StorageValue {
    fn resolve(&self, deployed: &HashMap<String, String>) -> Result<String> {
        match self {
            StorageValue::String(s) => {
                // Check if it's a reference to another contract
                if let Some(name) = s.strip_prefix('#') {
                    deployed
                        .get(name)
                        .cloned()
                        .ok_or_else(|| eyre!("Reference to unknown contract: #{}", name))
                } else {
                    Ok(s.clone())
                }
            }
            StorageValue::Number(n) => Ok(n.to_string()),
        }
    }

    fn to_hex_string(&self, deployed: &HashMap<String, String>) -> Result<String> {
        let resolved = self.resolve(deployed)?;

        if resolved.starts_with("0x") {
            let stripped = resolved.trim_start_matches("0x");
            Ok(format!("0x{:0>64}", stripped))
        } else if let Ok(num) = resolved.parse::<u64>() {
            Ok(format!("0x{:0>64x}", num))
        } else {
            Ok(format!("0x{:0>64}", resolved))
        }
    }
}

impl ConstructorArg {
    fn resolve_value(&self, deployed: &HashMap<String, String>) -> Result<String> {
        if let Some(name) = self.value.strip_prefix('#') {
            deployed
                .get(name)
                .cloned()
                .ok_or_else(|| eyre!("Reference to unknown contract in constructor: #{}", name))
        } else {
            Ok(self.value.clone())
        }
    }
}

impl ChainConfig {
    fn get_address_by_name(&self, name: &str) -> Option<String> {
        self.contracts
            .iter()
            .find(|c| c.name.as_deref() == Some(name))
            .and_then(|c| c.address.clone())
    }
}

fn normalize_slot(slot: &str) -> String {
    if slot.starts_with("0x") {
        let stripped = slot.trim_start_matches("0x");
        format!("0x{:0>64}", stripped)
    } else {
        if let Ok(num) = slot.parse::<u64>() {
            format!("0x{:0>64x}", num)
        } else {
            format!("0x{:0>64}", slot)
        }
    }
}

fn load_config(config_path: &PathBuf) -> Result<Option<ChainConfig>> {
    if !config_path.exists() {
        debug!("Config file not found at {:?}, skipping", config_path);
        return Ok(None);
    }

    let content =
        fs::read_to_string(config_path).map_err(|e| eyre!("Failed to read config file: {}", e))?;

    let config: ChainConfig =
        toml::from_str(&content).map_err(|e| eyre!("Failed to parse config file: {}", e))?;

    info!("Loaded config from {:?}", config_path);
    Ok(Some(config))
}

/// Load deployed bytecode from JSON artifact (for anvil_setCode)
fn load_deployed_bytecode(bytecode_path: &str) -> Result<String> {
    let content = fs::read_to_string(bytecode_path)
        .map_err(|e| eyre!("Failed to read bytecode file {}: {}", bytecode_path, e))?;

    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
        // Try Foundry/Hardhat format: deployedBytecode.object
        if let Some(bytecode) = json
            .get("deployedBytecode")
            .and_then(|b| b.get("object"))
            .and_then(|o| o.as_str())
        {
            return Ok(bytecode.to_string());
        }

        // Try Brownie format: deployedBytecode
        if let Some(bytecode) = json.get("deployedBytecode").and_then(|b| b.as_str()) {
            return Ok(bytecode.to_string());
        }
    }

    Ok(content.trim().to_string())
}

/// Load creation bytecode from JSON artifact
fn load_creation_bytecode(bytecode_path: &str) -> Result<String> {
    let content = fs::read_to_string(bytecode_path)
        .map_err(|e| eyre!("Failed to read bytecode file {}: {}", bytecode_path, e))?;

    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
        // Try Foundry/Hardhat format: bytecode.object
        if let Some(bytecode) = json
            .get("bytecode")
            .and_then(|b| b.get("object"))
            .and_then(|o| o.as_str())
        {
            return Ok(bytecode.to_string());
        }
        // Try Brownie format: bytecode
        else if let Some(bytecode) = json.get("bytecode").and_then(|b| b.as_str()) {
            return Ok(bytecode.to_string());
        }
    }

    let bytecode = content.trim().to_string();
    if bytecode.is_empty() {
        return Err(eyre!(
            "Could not find creation bytecode in {}",
            bytecode_path
        ));
    }

    Ok(bytecode)
}

/// Encode constructor arguments using alloy
fn encode_constructor_args(
    args: &[ConstructorArg],
    deployed: &HashMap<String, String>,
) -> Result<String> {
    use alloy::primitives::{Address, Bytes, U256};
    use alloy::sol_types::SolValue;

    let mut encoded = Vec::new();

    for arg in args {
        let resolved_value = arg.resolve_value(deployed)?;

        let token_bytes = match arg.arg_type.as_str() {
            "address" => {
                let addr: Address = resolved_value
                    .parse()
                    .map_err(|e| eyre!("Invalid address '{}': {}", resolved_value, e))?;
                addr.abi_encode()
            }
            "uint256" | "uint" => {
                let value = if resolved_value.starts_with("0x") {
                    U256::from_str_radix(&resolved_value[2..], 16)?
                } else {
                    U256::from_str_radix(&resolved_value, 10)?
                };
                value.abi_encode()
            }
            "string" => resolved_value.abi_encode(),
            "bytes" => {
                let data = resolved_value.trim_start_matches("0x");
                let bytes = hex::decode(data)
                    .map_err(|e| eyre!("Invalid hex bytes '{}': {}", resolved_value, e))?;
                Bytes::from(bytes).abi_encode()
            }
            "bool" => {
                let b = resolved_value
                    .parse::<bool>()
                    .map_err(|e| eyre!("Invalid bool value '{}': {}", resolved_value, e))?;
                b.abi_encode()
            }
            _ => return Err(eyre!("Unsupported constructor arg type: {}", arg.arg_type)),
        };
        encoded.extend_from_slice(&token_bytes);
    }

    Ok(format!("0x{}", hex::encode(encoded)))
}

/// Encode function call with arguments
fn encode_function_call(
    function_sig: &str,
    args: &[ConstructorArg],
    deployed: &HashMap<String, String>,
) -> Result<String> {
    use alloy::primitives::keccak256;

    // Calculate function selector (first 4 bytes of keccak256 hash)
    let selector = &keccak256(function_sig.as_bytes())[0..4];

    // Encode arguments
    let encoded_args = if args.is_empty() {
        String::new()
    } else {
        encode_constructor_args(args, deployed)?
            .trim_start_matches("0x")
            .to_string()
    };

    Ok(format!("0x{}{}", hex::encode(selector), encoded_args))
}

#[instrument(level = "trace", skip_all)]
async fn get_nonce(port: u16, client: &Client, address: &str) -> Result<u64> {
    let url = format!("http://localhost:{}", port);
    let request_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_getTransactionCount",
        "params": [address, "latest"],
        "id": 1
    });
    let response: serde_json::Value = client
        .post(&url)
        .json(&request_body)
        .send()
        .await?
        .json()
        .await?;

    let nonce_hex = response["result"]
        .as_str()
        .ok_or_else(|| eyre!("Invalid nonce response"))?
        .trim_start_matches("0x");

    let nonce = u64::from_str_radix(nonce_hex, 16)?;
    Ok(nonce)
}

#[instrument(level = "trace", skip_all)]
async fn execute_transaction(
    port: u16,
    client: &Client,
    from: &str,
    to: Option<&str>,
    data: &str,
    nonce: u64,
) -> Result<String> {
    let url = format!("http://localhost:{}", port);

    let mut params = serde_json::json!({
        "from": from,
        "data": data,
        "nonce": format!("0x{:x}", nonce),
        "gas": "0x500000",
    });

    if let Some(to_addr) = to {
        params["to"] = serde_json::json!(to_addr);
    }

    let request_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_sendTransaction",
        "params": [params],
        "id": 1
    });

    let res: serde_json::Value = client
        .post(&url)
        .json(&request_body)
        .send()
        .await?
        .json()
        .await?;

    if let Some(result) = res.get("result") {
        if let Some(result) = result.as_str() {
            let result = result.to_string();
            return Ok(result);
        }
        return Err(eyre!("unexpected result: {res}"));
    }
    if let Some(error) = res.get("error") {
        return Err(eyre!("{error}"));
    }
    return Err(eyre!("unexpected response: {res}"));
}

#[instrument(level = "trace", skip_all)]
async fn get_transaction_receipt(
    port: u16,
    client: &Client,
    tx_hash: &str,
) -> Result<Option<String>> {
    let url = format!("http://localhost:{}", port);
    let request_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_getTransactionReceipt",
        "params": [tx_hash],
        "id": 1
    });

    let response: serde_json::Value = client
        .post(&url)
        .json(&request_body)
        .send()
        .await?
        .json()
        .await?;

    if let Some(receipt) = response.get("result") {
        if receipt.is_null() {
            return Ok(None);
        }
        if let Some(contract_address) = receipt.get("contractAddress").and_then(|a| a.as_str()) {
            return Ok(Some(contract_address.to_string()));
        }
    }

    Ok(None)
}

#[instrument(level = "trace", skip_all)]
async fn deploy_contracts(port: u16, config: &ChainConfig) -> Result<HashMap<String, String>> {
    let client = Client::new();
    let url = format!("http://localhost:{}", port);

    let mut deployed_addresses = HashMap::new();

    // First, collect addresses from config (contracts with explicit address)
    for contract in &config.contracts {
        if let (Some(name), Some(address)) = (&contract.name, &contract.address) {
            deployed_addresses.insert(name.clone(), address.clone());
            info!("Pre-registered contract '{}' at {}", name, address);
        }
    }

    // Impersonate owner
    let request_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "anvil_impersonateAccount",
        "params": [OWNER_ADDRESS],
        "id": 1
    });
    let _: serde_json::Value = client
        .post(&url)
        .json(&request_body)
        .send()
        .await?
        .json()
        .await?;

    let mut nonce = get_nonce(port, &client, OWNER_ADDRESS).await?;

    // Deploy contracts sequentially, updating deployed_addresses after each
    for contract in &config.contracts {
        if let Some(json_path) = &contract.contract_json_path {
            let name = contract.name.as_deref().unwrap_or("unnamed");

            // Load creation bytecode
            let mut bytecode = load_creation_bytecode(json_path)?;

            // Append constructor args if any (with reference resolution)
            if !contract.constructor_args.is_empty() {
                let encoded_args =
                    encode_constructor_args(&contract.constructor_args, &deployed_addresses)?;
                let bytecode_clean = bytecode.trim_start_matches("0x");
                let encoded_args_clean = encoded_args.trim_start_matches("0x");
                bytecode = format!("0x{}{}", bytecode_clean, encoded_args_clean);
            }

            info!(
                "Deploying contract '{}' with bytecode length: {}",
                name,
                bytecode.len()
            );

            // Deploy (to = None for contract creation)
            match execute_transaction(port, &client, OWNER_ADDRESS, None, &bytecode, nonce).await {
                Ok(tx_hash) => {
                    info!("Deployment tx for '{}': {}", name, tx_hash);

                    // Wait for receipt
                    for _ in 0..10 {
                        sleep(Duration::from_millis(100)).await;
                        if let Ok(Some(contract_address)) =
                            get_transaction_receipt(port, &client, &tx_hash).await
                        {
                            info!("Contract '{}' deployed at: {}", name, contract_address);
                            if let Some(name) = &contract.name {
                                deployed_addresses.insert(name.clone(), contract_address);
                            }
                            break;
                        }
                    }
                }
                Err(e) => {
                    info!("Failed to deploy contract '{}': {}", name, e);
                }
            }

            nonce += 1;
        }
    }

    // Stop impersonating
    let request_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "anvil_stopImpersonatingAccount",
        "params": [OWNER_ADDRESS],
        "id": 1
    });
    let _: serde_json::Value = client
        .post(&url)
        .json(&request_body)
        .send()
        .await?
        .json()
        .await?;

    Ok(deployed_addresses)
}

#[instrument(level = "trace", skip_all)]
async fn execute_config_transactions(
    port: u16,
    config: &ChainConfig,
    deployed: &HashMap<String, String>,
) -> Result<()> {
    let client = Client::new();
    let url = format!("http://localhost:{}", port);

    if config.transactions.is_empty() {
        return Ok(());
    }

    // Impersonate owner
    let request_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "anvil_impersonateAccount",
        "params": [OWNER_ADDRESS],
        "id": 1
    });
    let _: serde_json::Value = client
        .post(&url)
        .json(&request_body)
        .send()
        .await?
        .json()
        .await?;

    let mut nonce = get_nonce(port, &client, OWNER_ADDRESS).await?;

    for tx_config in &config.transactions {
        let name = tx_config.name.as_deref().unwrap_or("unnamed");

        // Resolve target address (might be a reference)
        let target = if let Some(ref_name) = tx_config.target.strip_prefix('#') {
            deployed.get(ref_name).cloned().ok_or_else(|| {
                eyre!(
                    "Reference to unknown contract in transaction target: #{}",
                    ref_name
                )
            })?
        } else {
            tx_config.target.clone()
        };

        // Build transaction data
        let data = if let Some(inline_data) = &tx_config.data {
            inline_data.clone()
        } else if let Some(function_sig) = &tx_config.function_signature {
            encode_function_call(function_sig, &tx_config.args, deployed)?
        } else {
            return Err(eyre!(
                "Transaction '{}' must have either 'data' or 'function_signature'",
                name
            ));
        };

        info!("Executing transaction '{}' to {}", name, target);

        match execute_transaction(port, &client, OWNER_ADDRESS, Some(&target), &data, nonce).await {
            Ok(tx_hash) => {
                info!("Transaction '{}' sent: {}", name, tx_hash);
            }
            Err(e) => {
                info!("Transaction '{}' failed: {}", name, e);
            }
        }

        nonce += 1;
    }

    // Stop impersonating
    let request_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "anvil_stopImpersonatingAccount",
        "params": [OWNER_ADDRESS],
        "id": 1
    });
    let _: serde_json::Value = client
        .post(&url)
        .json(&request_body)
        .send()
        .await?
        .json()
        .await?;

    Ok(())
}

#[instrument(level = "trace", skip_all)]
async fn initialize_contracts(port: u16, addresses: &ContractAddresses) -> Result<()> {
    let client = Client::new();
    let url = format!("http://localhost:{}", port);

    let request_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "anvil_impersonateAccount",
        "params": [OWNER_ADDRESS],
        "id": 1
    });
    let _: serde_json::Value = client
        .post(&url)
        .json(&request_body)
        .send()
        .await?
        .json()
        .await?;

    let mut nonce = get_nonce(port, &client, OWNER_ADDRESS).await?;

    let transactions = addresses.get_transactions();
    for (to, data) in transactions {
        match execute_transaction(port, &client, OWNER_ADDRESS, Some(&to), &data, nonce).await {
            Ok(result) => info!("Transaction to {to}:  {result}"),
            Err(e) => info!("Transaction failed: {e:?}"),
        }
        nonce += 1;
    }

    let request_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "anvil_stopImpersonatingAccount",
        "params": [OWNER_ADDRESS],
        "id": 1
    });
    let _: serde_json::Value = client
        .post(&url)
        .json(&request_body)
        .send()
        .await?
        .json()
        .await?;

    Ok(())
}

#[instrument(level = "trace", skip_all)]
async fn apply_config_contracts(
    port: u16,
    config: &ChainConfig,
    deployed: &HashMap<String, String>,
) -> Result<()> {
    let client = Client::new();
    let url = format!("http://localhost:{}", port);

    // Only process contracts with explicit address (not deployed via contract_json_path)
    for contract in &config.contracts {
        // Skip if this is a deployment contract
        if contract.contract_json_path.is_some() {
            continue;
        }

        let Some(address) = &contract.address else {
            continue;
        };

        let bytecode = if let Some(inline_bytecode) = &contract.bytecode {
            Some(inline_bytecode.clone())
        } else if let Some(deployed_path) = &contract.deployed_bytecode_path {
            Some(load_deployed_bytecode(deployed_path)?)
        } else {
            None
        };

        if let Some(bytecode) = bytecode {
            let request_body = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "anvil_setCode",
                "params": [address, bytecode.trim()],
                "id": 1
            });
            let _: serde_json::Value = client
                .post(&url)
                .json(&request_body)
                .send()
                .await?
                .json()
                .await?;

            let name_info = contract.name.as_deref().unwrap_or("unnamed");
            info!("Set bytecode for contract '{}' at {}", name_info, address);
        }

        // Apply storage with reference resolution
        for (slot, value) in &contract.storage {
            let normalized_slot = normalize_slot(slot);
            let hex_value = value.to_hex_string(deployed)?;

            let request_body = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "anvil_setStorageAt",
                "params": [address, normalized_slot, hex_value],
                "id": 1
            });
            let _: serde_json::Value = client
                .post(&url)
                .json(&request_body)
                .send()
                .await?
                .json()
                .await?;
            debug!("Set storage slot {} for {} to {}", slot, address, hex_value);
        }
    }

    Ok(())
}

#[instrument(level = "trace", skip_all)]
async fn check_dot_os_tba(port: u16, addresses: &ContractAddresses) -> Result<bool> {
    let client = Client::new();
    let url = format!("http://localhost:{}", port);

    let request_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_getCode",
        "params": [&addresses.dot_os_tba, "latest"],
        "id": 1
    });

    let response = client.post(&url).json(&request_body).send().await?;
    let result: serde_json::Value = response.json().await?;
    let code = result["result"].as_str().unwrap_or("0x");
    Ok(code != "0x")
}

#[instrument(level = "trace", skip_all)]
pub async fn start_chain(
    port: u16,
    recv_kill: BroadcastRecvBool,
    verbose: bool,
    tracing: bool,
) -> Result<Option<Child>> {
    start_chain_with_config(port, recv_kill, verbose, tracing, None).await
}

#[instrument(level = "trace", skip_all)]
pub async fn start_chain_with_config(
    port: u16,
    mut recv_kill: BroadcastRecvBool,
    verbose: bool,
    tracing: bool,
    custom_config_path: Option<PathBuf>,
) -> Result<Option<Child>> {
    let deps = check_foundry_deps()?;
    get_deps(
        deps,
        &mut recv_kill,
        false,
        verbose,
        build::DEFAULT_RUST_TOOLCHAIN,
    )
    .await?;

    let default_config = load_config(&PathBuf::from(DEFAULT_CONFIG_PATH))?;

    let custom_config = if let Some(path) = custom_config_path {
        load_config(&path)?
    } else {
        None
    };

    // Use custom config if available, otherwise default
    let active_config = custom_config
        .as_ref()
        .or(default_config.as_ref())
        .ok_or_else(|| {
            eyre!(
                "No config file found. Please provide {} or specify a custom config path",
                DEFAULT_CONFIG_PATH
            )
        })?;

    info!("Checking for Anvil on port {}...", port);
    if wait_for_anvil(port, 1, None).await.is_ok() {
        // Deploy contracts first (sequentially with reference resolution)
        let mut deployed_addresses = HashMap::new();
        if let Some(config) = default_config.as_ref() {
            deployed_addresses.extend(deploy_contracts(port, config).await?);
        }
        if let Some(config) = custom_config.as_ref() {
            deployed_addresses.extend(deploy_contracts(port, config).await?);
        }

        // Load contract addresses (now with deployed addresses)
        let addresses =
            ContractAddresses::from_config(port, active_config, &deployed_addresses).await?;
        info!("Loaded contract addresses from config");

        if !check_dot_os_tba(port, &addresses).await? {
            // Apply bytecode at known addresses (with reference resolution)
            if let Some(config) = default_config.as_ref() {
                apply_config_contracts(port, config, &deployed_addresses).await?;
            }
            if let Some(config) = custom_config.as_ref() {
                apply_config_contracts(port, config, &deployed_addresses).await?;
            }

            // Execute config transactions
            if let Some(config) = default_config.as_ref() {
                execute_config_transactions(port, config, &deployed_addresses).await?;
            }
            if let Some(config) = custom_config.as_ref() {
                execute_config_transactions(port, config, &deployed_addresses).await?;
            }

            // Finally initialize
            initialize_contracts(port, &addresses).await?;
        }

        // Print summary
        addresses.print_summary();

        return Ok(None);
    }

    let mut args = vec!["--port".to_string(), port.to_string()];
    if tracing {
        args.push("--tracing".to_string());
    }
    let mut child = Command::new("anvil")
        .args(args)
        .current_dir(KIT_CACHE)
        .stdout(if verbose {
            Stdio::inherit()
        } else {
            Stdio::piped()
        })
        .spawn()?;

    info!("Waiting for Anvil to be ready on port {}...", port);
    if let Err(e) = wait_for_anvil(port, DEFAULT_MAX_ATTEMPTS, Some(recv_kill)).await {
        let _ = child.kill();
        return Err(e);
    }

    // Deploy contracts first (sequentially with reference resolution)
    let mut deployed_addresses = HashMap::new();
    if let Some(config) = default_config.as_ref() {
        match deploy_contracts(port, config).await {
            Ok(addrs) => deployed_addresses.extend(addrs),
            Err(e) => {
                let _ = child.kill();
                return Err(e.wrap_err("Failed to deploy contracts from default config"));
            }
        }
    }
    if let Some(config) = custom_config.as_ref() {
        match deploy_contracts(port, config).await {
            Ok(addrs) => deployed_addresses.extend(addrs),
            Err(e) => {
                let _ = child.kill();
                return Err(e.wrap_err("Failed to deploy contracts from custom config"));
            }
        }
    }

    // Load contract addresses (now with deployed addresses)
    let addresses =
        ContractAddresses::from_config(port, active_config, &deployed_addresses).await?;
    info!("Loaded contract addresses from config");

    if !check_dot_os_tba(port, &addresses).await? {
        // Apply bytecode at known addresses (with reference resolution)
        if let Some(config) = default_config.as_ref() {
            if let Err(e) = apply_config_contracts(port, &config, &deployed_addresses).await {
                let _ = child.kill();
                return Err(e.wrap_err("Failed to apply default config contracts"));
            }
        }
        if let Some(config) = custom_config.as_ref() {
            if let Err(e) = apply_config_contracts(port, &config, &deployed_addresses).await {
                let _ = child.kill();
                return Err(e.wrap_err("Failed to apply custom config contracts"));
            }
        }

        // Execute config transactions
        if let Some(config) = default_config.as_ref() {
            if let Err(e) = execute_config_transactions(port, &config, &deployed_addresses).await {
                let _ = child.kill();
                return Err(e.wrap_err("Failed to execute default config transactions"));
            }
        }
        if let Some(config) = custom_config.as_ref() {
            if let Err(e) = execute_config_transactions(port, &config, &deployed_addresses).await {
                let _ = child.kill();
                return Err(e.wrap_err("Failed to execute custom config transactions"));
            }
        }

        // Finally initialize
        if let Err(e) = initialize_contracts(port, &addresses).await {
            let _ = child.kill();
            return Err(e.wrap_err("Failed to initialize contracts"));
        }
    }

    if let Err(e) = verify_contracts(port, &addresses).await {
        let _ = child.kill();
        return Err(e.wrap_err("Contract verification failed"));
    }

    // Print summary
    addresses.print_summary();

    Ok(Some(child))
}

#[instrument(level = "trace", skip_all)]
async fn wait_for_anvil(
    port: u16,
    max_attempts: u16,
    mut recv_kill: Option<BroadcastRecvBool>,
) -> Result<()> {
    let client = Client::new();
    let url = format!("http://localhost:{}", port);

    for _ in 0..max_attempts {
        let request_body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_blockNumber",
            "params": [],
            "id": 1
        });

        let response = client.post(&url).json(&request_body).send().await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                let result: serde_json::Value = resp.json().await?;
                if let Some(block_number) = result["result"].as_str() {
                    if block_number.starts_with("0x") {
                        info!("Anvil is ready on port {}.", port);
                        return Ok(());
                    }
                }
            }
            _ => (),
        }

        if let Some(ref mut recv_kill) = recv_kill {
            tokio::select! {
                _ = sleep(Duration::from_millis(250)) => {}
                _ = recv_kill.recv() => {
                    return Err(eyre!("Received kill: bringing down anvil."));
                }
            }
        } else {
            sleep(Duration::from_millis(250)).await;
        }
    }

    Err(eyre!(
        "Failed to connect to Anvil on port {} after {} attempts",
        port,
        max_attempts
    )
    .with_suggestion(|| "Is port already occupied?"))
}

#[instrument(level = "trace", skip_all)]
pub async fn call_contract(port: u16, target: &str, data: &str) -> Result<String> {
    let client = Client::new();
    let url = format!("http://localhost:{}", port);

    let request_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_call",
        "params": [{
            "to": target,
            "data": data,
        }, "latest"],
        "id": 1
    });

    let response = client.post(&url).json(&request_body).send().await?;

    let result: serde_json::Value = response.json().await?;

    if let Some(output) = result.get("result").and_then(|r| r.as_str()) {
        return Ok(output.to_string());
    }

    if let Some(error) = result.get("error") {
        return Err(eyre!("Contract call failed: {}", error));
    }

    Err(eyre!("Unexpected response: {}", result))
}

#[instrument(level = "trace", skip_all)]
pub async fn verify_contracts(port: u16, addresses: &ContractAddresses) -> Result<()> {
    info!("Verifying deployed contracts...");

    // cast calldata "symbol()"
    let symbol_calldata = "0x95d89b41";
    match call_contract(port, &addresses.hypermap_proxy, symbol_calldata).await {
        Ok(result) => {
            info!("Hypermap symbol: {}", result);
        }
        Err(e) => {
            return Err(e.wrap_err("Failed to verify Hypermap contract"));
        }
    }

    info!("All contracts verified successfully");
    Ok(())
}

#[instrument(level = "trace", skip_all)]
pub async fn execute(
    port: u16,
    verbose: bool,
    tracing: bool,
    custom_config_path: Option<PathBuf>,
) -> Result<()> {
    let (send_to_cleanup, mut recv_in_cleanup) = tokio::sync::mpsc::unbounded_channel();
    let (send_to_kill, _recv_kill) = tokio::sync::broadcast::channel(1);
    let recv_kill_in_cos = send_to_kill.subscribe();

    let handle_signals = tokio::spawn(cleanup_on_signal(send_to_cleanup.clone(), recv_kill_in_cos));

    let recv_kill_in_start_chain = send_to_kill.subscribe();
    let child = start_chain_with_config(
        port,
        recv_kill_in_start_chain,
        verbose,
        tracing,
        custom_config_path,
    )
    .await?;
    let Some(mut child) = child else {
        return Err(eyre!(
            "Port {} is already in use by another anvil process",
            port
        ));
    };
    let child_id = child.id() as i32;

    let cleanup_anvil = tokio::spawn(async move {
        recv_in_cleanup.recv().await;
        clean_process_by_pid(child_id);
    });

    let _ = child.wait();

    let _ = handle_signals.await;
    let _ = cleanup_anvil.await;

    let _ = send_to_kill.send(true);

    Ok(())
}
