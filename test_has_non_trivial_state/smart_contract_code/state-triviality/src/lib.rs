// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

/*! ABI of the State Triviality Example Application that does not use GraphQL */

use linera_sdk::linera_base_types::{ContractAbi, ServiceAbi};
use serde::{Deserialize, Serialize};

pub struct StateTrivialityAbi;

impl ContractAbi for StateTrivialityAbi {
    type Operation = StateTrivialityOperation;
    type Response = ();
}

impl ServiceAbi for StateTrivialityAbi {
    type Query = StateTrivialityRequest;
    type QueryResponse = u64;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum StateTrivialityRequest {
    Query,
    CreateAndCall(Vec<u8>, Vec<u8>, u64, bool),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum StateTrivialityOperation {
    CreateAndCall(Vec<u8>, Vec<u8>, u64, bool),
    TestTrivialState(bool),
}
