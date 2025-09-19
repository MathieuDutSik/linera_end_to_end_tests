// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use counter_no_state::{CounterNoStateAbi, CounterOperation};
use linera_sdk::{
    linera_base_types::WithContractAbi,
    views::{RootView, View},
    Contract, ContractRuntime,
};

use self::state::CounterState;

pub struct CounterContract {
    state: CounterState,
}

linera_sdk::contract!(CounterContract);

impl WithContractAbi for CounterContract {
    type Abi = CounterNoStateAbi;
}

impl Contract for CounterContract {
    type Message = ();
    type InstantiationArgument = ();
    type Parameters = ();
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = CounterState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        CounterContract { state }
    }

    async fn instantiate(&mut self, _value: ()) {
        // Nothing to do in instantiate
    }

    async fn execute_operation(&mut self, operation: CounterOperation) -> u64 {
        let previous_value = self.state.value.get();
        let CounterOperation::Increment(value) = operation;
        let new_value = previous_value + value;
        self.state.value.set(new_value);
        new_value
    }

    async fn execute_message(&mut self, _message: ()) {
        panic!("Counter application doesn't support any cross-chain messages");
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}
