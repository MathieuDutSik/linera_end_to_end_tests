// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

/*! ABI of the Counter Example Application */

use async_graphql::{Request, Response};
use linera_sdk::{
    graphql::GraphQLMutationRoot,
    linera_base_types::{ContractAbi, ServiceAbi},
};
use serde::{Deserialize, Serialize};

pub struct ReportedSolutionsAbi;

#[derive(Debug, Deserialize, Serialize, GraphQLMutationRoot)]
pub enum ReportedSolutionsOperation {
    InsertEntry { key1: String, key2: String, value: u64 },
}

impl ContractAbi for ReportedSolutionsAbi {
    type Operation = ReportedSolutionsOperation;
    type Response = ();
}

impl ServiceAbi for ReportedSolutionsAbi {
    type Query = Request;
    type QueryResponse = Response;
}
