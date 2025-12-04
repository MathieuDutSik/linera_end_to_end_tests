// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Code for compiling solidity smart contracts for testing purposes.

use std::{
//    fs::File,
    collections::HashMap,
//    io::Write,
//    path::Path,
    path::PathBuf,
//    process::{Command, Stdio},
};

use anyhow::Context;
use linera_sdk::abis::evm::EvmAbi;
use linera_base::{
    identifiers::ApplicationId,
    vm::EvmInstantiation,
    vm::VmRuntime,
};
use linera_service::cli_wrappers::ClientWrapper;

//use revm_primitives::{Address, U256};
//use tempfile::tempdir;
use tempfile::TempDir;

// Linera Solidity library constants
//const LINERA_SOL: &str = include_str!("../solidity/Linera.sol");
//const LINERA_TYPES_SOL: &str = include_str!("../solidity/LineraTypes.sol");

pub async fn publish_evm_contract(client: &ClientWrapper, data_contract: &serde_json::Value, constructor_argument: &Vec<u8>, evm_instantiation: &EvmInstantiation) -> anyhow::Result<ApplicationId<EvmAbi>> {
    let evm_data = data_contract
        .get("evm")
        .with_context(|| format!("failed to get evm in data_contract={data_contract}"))?;
    let bytecode = evm_data
        .get("bytecode")
        .with_context(|| format!("failed to get bytecode in evm_data={evm_data}"))?;
    let object = bytecode
        .get("object")
        .with_context(|| format!("failed to get object in bytecode={bytecode}"))?;
    let object = object.to_string();
    let object = object.trim_matches(|c| c == '"').to_string();
    let module = hex::decode(&object)?;
    let (evm_contract, _dir) = temporary_write_evm_module(module)?;
    Ok(client
        .publish_and_create::<EvmAbi, Vec<u8>, EvmInstantiation>(
            evm_contract.clone(),
            evm_contract,
            VmRuntime::Evm,
            constructor_argument,
            evm_instantiation,
            &[],
            None,
        )
        .await?)
}



pub async fn read_and_publish_contracts(client: &ClientWrapper, path: &PathBuf, file_name: &str, contract_name: &str, map: &HashMap<(String, String), (Vec<u8>, EvmInstantiation)>) -> anyhow::Result<ApplicationId<EvmAbi>> {
    println!("read_bytecode_from_file, path={}", path.display());
    let contents = std::fs::read_to_string(path)?;
    let json_data: serde_json::Value = serde_json::from_str(&contents)?;
    let contracts = json_data
        .get("contracts")
        .and_then(|c| c.as_object())
        .with_context(|| format!("contracts is missing or not an object: {json_data}"))?;
    let file_name_keys: Vec<&String> = contracts.keys().collect();
    let n_file = file_name_keys.len();
    println!("file_name_keys={file_name_keys:?}");
    let mut return_application_id = None;
    let mut n_application = 0;
    for (i_file, file_name_key) in file_name_keys.into_iter().enumerate() {
        let contract_block = contracts
            .get(file_name_key)
            .with_context(|| format!("failed to get {file_name_key}"))?;
        let contract_keys: Vec<&String> = contract_block.as_object().expect("A m-a-p").keys().collect();
        let n_contract = contract_keys.len();
        for (i_contract, contract_key) in contract_keys.into_iter().enumerate() {
            println!("Processing {i_file}/{n_file} - {i_contract}/{n_contract} file_name_key={file_name_key} contract_key={contract_key}");
            let big_key: (String, String) = (file_name_key.clone(), contract_key.into());
            let (constructor_argument, instantiation_argument) = match map.get(&big_key) {
                None => (Vec::<u8>::new(), EvmInstantiation::default()),
                Some(const_inst) => const_inst.clone(),
            };
            let data_contract = contract_block
                .get(contract_key)
                .with_context(|| format!("failed to get contract_key={contract_key}"))?;
            let application_id = publish_evm_contract(client, data_contract, &constructor_argument, &instantiation_argument).await?;
            println!("contract_key={} contract_name={}", contract_key, contract_name);
            if file_name_key == file_name && contract_key == contract_name {
                println!("Mathing the test");
                return_application_id = Some(application_id);
            }
            n_application += 1;
        }
    }
    println!("read_and_publish_contracts n_application={n_application}");
    let application_id = return_application_id.expect("We were unable to find contract_name in the list");
    Ok(application_id)
}


pub async fn read_and_publish_contract(client: &ClientWrapper, path: &PathBuf, file_name: &str, contract_name: &str, constructor_argument: Vec<u8>, evm_instantiation: EvmInstantiation) -> anyhow::Result<ApplicationId<EvmAbi>> {
    println!("read_bytecode_from_file, path={}", path.display());
    let contents = std::fs::read_to_string(path)?;
    let json_data: serde_json::Value = serde_json::from_str(&contents)?;
    let contracts = json_data
        .get("contracts")
        .and_then(|c| c.as_object())
        .with_context(|| format!("contracts is missing or not an object: {json_data}"))?;
    let contract_block = contracts
        .get(file_name)
        .with_context(|| format!("failed to get {file_name}"))?;
    let data_contract = contract_block
        .get(contract_name)
        .with_context(|| format!("failed to get contract_name={contract_name}"))?;
    publish_evm_contract(client, data_contract, &constructor_argument, &evm_instantiation).await
}



pub fn temporary_write_evm_module(module: Vec<u8>) -> anyhow::Result<(PathBuf, TempDir)> {
    let dir = tempfile::tempdir()?;
    let path = dir.path();
    let app_file = "app.json";
    let app_path = path.join(app_file);
    {
        std::fs::write(app_path.clone(), &module)?;
    }
    let evm_contract = app_path.to_path_buf();
    Ok((evm_contract, dir))
}



/*
fn write_compilation_json(path: &Path, file_name: &str) -> anyhow::Result<()> {
    let mut source = File::create(path).unwrap();
    writeln!(
        source,
        r#"
{{
  "language": "Solidity",
  "sources": {{
    "{file_name}": {{
      "urls": ["./{file_name}"]
    }}
  }},
  "settings": {{
    "viaIR": true,
    "outputSelection": {{
      "*": {{
        "*": ["evm.bytecode"]
      }}
    }}
  }}
}}
"#
    )?;
    Ok(())
}
*/

pub fn read_bytecode_from_file(path: &PathBuf, file_name: &str, contract_name: &str) -> anyhow::Result<Vec<u8>> {
    println!("read_bytecode_from_file, path={}", path.display());
    let contents = std::fs::read_to_string(path)?;
    let json_data: serde_json::Value = serde_json::from_str(&contents)?;
    let contracts = json_data
        .get("contracts")
        .with_context(|| format!("failed to get contracts in json_data={json_data}"))?;
    let file_name_contract = contracts
        .get(file_name)
        .with_context(|| format!("failed to get {file_name}"))?;
    let test_data = file_name_contract
        .get(contract_name)
        .with_context(|| format!("failed to get contract_name={contract_name}"))?;
    let evm_data = test_data
        .get("evm")
        .with_context(|| format!("failed to get evm in test_data={test_data}"))?;
    let bytecode = evm_data
        .get("bytecode")
        .with_context(|| format!("failed to get bytecode in evm_data={evm_data}"))?;
    let object = bytecode
        .get("object")
        .with_context(|| format!("failed to get object in bytecode={bytecode}"))?;
    let object = object.to_string();
    let object = object.trim_matches(|c| c == '"').to_string();
    Ok(hex::decode(&object)?)
}

/*
fn get_bytecode_path(path: &Path, file_name: &str, contract_name: &str) -> anyhow::Result<Vec<u8>> {
    let config_path = path.join("config.json");
    write_compilation_json(&config_path, file_name)?;
    let config_file = File::open(config_path)?;

    let output_path = path.join("result.json");
    let output_file = File::create(output_path.clone())?;

    let status = Command::new("solc")
        .current_dir(path)
        .arg("--standard-json")
        .stdin(Stdio::from(config_file))
        .stdout(Stdio::from(output_file))
        .status()?;
    assert!(status.success());

    read_bytecode_from_file(&output_path, file_name, contract_name)
}
*/

/*
pub fn get_bytecode(source_code: &str, contract_name: &str) -> anyhow::Result<Vec<u8>> {
    let dir = tempdir().unwrap();
    let path = dir.path();
    if source_code.contains("Linera.sol") {
        // The source code seems to import Linera.sol, so we import the relevant files.
        for (file_name, literal_path) in [
            ("Linera.sol", LINERA_SOL),
            ("LineraTypes.sol", LINERA_TYPES_SOL),
        ] {
            let test_code_path = path.join(file_name);
            let mut test_code_file = File::create(&test_code_path)?;
            writeln!(test_code_file, "{}", literal_path)?;
        }
    }
    if source_code.contains("@openzeppelin") {
        let pwd: PathBuf = std::env::current_dir()?;
        tracing::info!("pwd={pwd:?}");
        let existing_dir: PathBuf = pwd.join("@openzeppelin");
        if existing_dir.exists() {
            let path_str: &str = path.to_str().unwrap();
            tracing::info!("path_str={path_str:?}");
            let _output = Command::new("cp")
                .args(["-r", "@openzeppelin", path_str])
                .output()?;
        } else {
            let _output = Command::new("npm")
                .args(["install", "@openzeppelin/contracts-upgradeable"])
                .current_dir(path)
                .output()?;
            let _output = Command::new("mv")
                .args(["node_modules/@openzeppelin", "@openzeppelin"])
                .current_dir(path)
                .output()?;
        }
    }
    let file_name = "test_code.sol";
    let test_code_path = path.join(file_name);
    let mut test_code_file = File::create(&test_code_path)?;
    writeln!(test_code_file, "{}", source_code)?;
    get_bytecode_path(path, file_name, contract_name)
}
*/

/*
pub fn load_solidity_example(path: &str) -> anyhow::Result<Vec<u8>> {
    let source_code = std::fs::read_to_string(path)?;
    let contract_name: &str = source_code
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("contract "))
        .next()
        .ok_or_else(|| anyhow::anyhow!("Not matching"))?;
    let contract_name: &str = contract_name
        .split_whitespace()
        .next()
        .ok_or(anyhow::anyhow!("No space found after the contract name"))?;
    tracing::info!("load_solidity_example, contract_name={contract_name}");
    get_bytecode(&source_code, contract_name)
}
*/

/*
pub fn load_solidity_example_by_name(path: &str, contract_name: &str) -> anyhow::Result<Vec<u8>> {
    let source_code = std::fs::read_to_string(path)?;
    get_bytecode(&source_code, contract_name)
}
*/

/*
pub fn get_evm_contract_path(path: &str) -> anyhow::Result<(PathBuf, TempDir)> {
    let module = load_solidity_example(path)?;
    temporary_write_evm_module(module)
}
*/

/*
pub fn value_to_vec_u8(value: Value) -> Vec<u8> {
    let mut vec: Vec<u8> = Vec::new();
    for val in value.as_array().unwrap() {
        let val = val.as_u64().unwrap();
        let val = val as u8;
        vec.push(val);
    }
    vec
}
*/

/*
pub fn read_evm_u64_entry(value: Value) -> u64 {
    let vec = value_to_vec_u8(value);
    let mut arr = [0_u8; 8];
    arr.copy_from_slice(&vec[24..]);
    u64::from_be_bytes(arr)
}
*/

/*
pub fn read_evm_u256_entry(value: Value) -> U256 {
    let result = value_to_vec_u8(value);
    U256::from_be_slice(&result)
}

pub fn read_evm_address_entry(value: Value) -> Address {
    let vec = value_to_vec_u8(value);
    let mut arr = [0_u8; 20];
    arr.copy_from_slice(&vec[12..]);
    Address::from_slice(&arr)
}
*/
