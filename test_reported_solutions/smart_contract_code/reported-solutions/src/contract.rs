// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use reported_solutions::{ReportedSolutionsAbi, ReportedSolutionsOperation, ReportedSolutionsOperation::*};
use linera_sdk::{
    linera_base_types::WithContractAbi,
    views::{RootView, View},
    Contract, ContractRuntime,
};

use self::state::ReportedSolutionsState;

pub struct ReportedSolutionsContract {
    state: ReportedSolutionsState,
}

linera_sdk::contract!(ReportedSolutionsContract);

impl WithContractAbi for ReportedSolutionsContract {
    type Abi = ReportedSolutionsAbi;
}

impl Contract for ReportedSolutionsContract {
    type Message = ();
    type InstantiationArgument = ();
    type Parameters = ();
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = ReportedSolutionsState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        ReportedSolutionsContract { state }
    }

    async fn instantiate(&mut self, _value: ()) {
    }

    async fn execute_operation(&mut self, operation: ReportedSolutionsOperation) {
        match operation {
            InsertEntry { key1, key2, value } => {
                let subview = self.state.reported_solutions.load_entry_mut(&key1).await.unwrap();
                subview.insert(&key2, value).unwrap();
            },
        }
    }

    async fn execute_message(&mut self, _message: ()) {
        panic!("Counter application doesn't support any cross-chain messages");
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}
