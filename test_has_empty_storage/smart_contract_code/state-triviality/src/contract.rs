// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use counter_no_state::{CounterNoStateAbi, CounterOperation, CounterRequest};
use state_triviality::{StateTrivialityAbi, StateTrivialityOperation};
use linera_sdk::{
    linera_base_types::{ApplicationId, Bytecode, CryptoHash, VmRuntime, WithContractAbi},
    views::{RootView, View},
    Contract, ContractRuntime,
};

use self::state::StateTrivialityState;

pub struct StateTrivialityContract {
    state: StateTrivialityState,
    runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(StateTrivialityContract);

impl WithContractAbi for StateTrivialityContract {
    type Abi = StateTrivialityAbi;
}

impl Contract for StateTrivialityContract {
    type Message = ();
    type InstantiationArgument = ();
    type Parameters = ();
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = StateTrivialityState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        StateTrivialityContract { state, runtime }
    }

    async fn instantiate(&mut self, _value: ()) {
        // Validate that the application parameters were configured correctly.
        self.runtime.application_parameters();

        self.state.app_id.set(None);
    }

    async fn execute_operation(&mut self, operation: StateTrivialityOperation) {
        match operation {
            StateTrivialityOperation::CreateAndCall(contract_bytes, service_bytes, increment_value, do_save) => {
                // Step 1: Convert Vec<u8> to Bytecode and publish module with Wasm runtime
                let contract_bytecode = Bytecode::new(contract_bytes);
                let service_bytecode = Bytecode::new(service_bytes);
                let module_id =
                    self.runtime
                    .publish_module(contract_bytecode, service_bytecode, VmRuntime::Wasm);

                // Step 2: Create application with initialization value
                let initialization_value = ();
                let application_id = self
                    .runtime
                    .create_application::<CounterNoStateAbi, (), ()>(
                        module_id,
                        &(),
                        &initialization_value,
                        vec![],
                    );
                self.state.app_id.set(Some(application_id));

                let app_id_untyped = application_id.forget_abi();
                let is_trivial = self.runtime.has_trivial_storage(app_id_untyped);
                assert!(is_trivial, "app_id_untyped should be trivial");

                let vec = [0_u8; 32];
                let hash = CryptoHash::from(vec);
                let app_id_not_exist = ApplicationId::new(hash);
                let is_trivial = self.runtime.has_trivial_storage(app_id_not_exist);
                assert!(is_trivial, "app_id_not_exist should be trivial");

                // Step 3: Call the service. It should return the value before
                // the initialization of this contract and thus zero.
                // A: operation
                let query_operation = CounterOperation::Query;
                let val = self.runtime.call_application(true, application_id, &query_operation);
                assert!(val == 0, "application_id starts at 0");

                // B: query
                let query_request = CounterRequest::Query;
                let val = self.runtime.query_service(application_id, &query_request);
                assert!(val == 0, "initial value should be zero");

                // Step 4: Call the contract with counter increment operation
                let counter_operation = CounterOperation::Increment(increment_value, do_save);
                self.runtime
                    .call_application(true, application_id, &counter_operation);

                // Step 5: Querying the value of the application
                // A: operation
                let query_operation = CounterOperation::Query;
                let val = self.runtime.call_application(true, application_id, &query_operation);
                assert!(val == increment_value, "application_id has been increased, visible in contracts");

                // B: query
                let val = self.runtime.query_service(application_id, &query_request);
                if do_save {
                    assert!(val == increment_value, "We save, so the value should be the incremented one");
                } else {
                    assert!(val == 0, "Not saved, therefore the service value is 0");
                }

                let is_trivial = self.runtime.has_trivial_storage(app_id_untyped);
                if do_save {
                    assert!(!is_trivial, "app_id_untyped should have non-trivial storage since it has been saved");
                } else {
                    assert!(is_trivial, "no save, therefore trivial state");
                }
            },
            StateTrivialityOperation::TestTrivialState(expected_value) => {
                let app_id = *self.state.app_id.get();
                let app_id: ApplicationId = app_id.unwrap().forget_abi();
                let is_trivial = self.runtime.has_trivial_storage(app_id);
                assert_eq!(is_trivial, expected_value);
            }
        }
    }

    async fn execute_message(&mut self, _message: ()) {
        panic!("State triviality application doesn't support any cross-chain messages");
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}
