// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

/*! ABI of the Counter Example Application that does not use GraphQL */

use linera_sdk::linera_base_types::{ContractAbi, ServiceAbi};
use serde::{Deserialize, Serialize};

pub struct CounterNoStateAbi;

impl ContractAbi for CounterNoStateAbi {
    type Operation = CounterOperation;
    type Response = u64;
}

impl ServiceAbi for CounterNoStateAbi {
    type Query = CounterRequest;
    type QueryResponse = u64;
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CounterRequest {
    Query,
    Increment(u64),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CounterOperation {
    Increment(u64),
}
