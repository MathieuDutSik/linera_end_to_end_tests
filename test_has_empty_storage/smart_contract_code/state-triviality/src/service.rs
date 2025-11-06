// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use std::sync::Arc;

use counter_no_state::CounterRequest;
use state_triviality::{StateTrivialityOperation, StateTrivialityRequest};
use linera_sdk::{linera_base_types::WithServiceAbi, views::View, Service, ServiceRuntime};

use self::state::StateTrivialityState;

pub struct StateTrivialityService {
    state: StateTrivialityState,
    runtime: Arc<ServiceRuntime<Self>>,
}

linera_sdk::service!(StateTrivialityService);

impl WithServiceAbi for StateTrivialityService {
    type Abi = state_triviality::StateTrivialityAbi;
}

impl Service for StateTrivialityService {
    type Parameters = ();

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        let state = StateTrivialityState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        StateTrivialityService {
            state,
            runtime: Arc::new(runtime),
        }
    }

    async fn handle_query(&self, request: StateTrivialityRequest) -> u64 {
        match request {
            StateTrivialityRequest::Query => {
                let application_id = self.state.app_id.get().expect("An application_id");
                let counter_request = CounterRequest::Query;
                self.runtime
                    .query_application(application_id, &counter_request)
            }
            StateTrivialityRequest::CreateAndCall(bytecode, calldata, increment, do_save) => {
                let operation = StateTrivialityOperation::CreateAndCall(
                    bytecode,
                    calldata,
                    increment,
                    do_save,
                );
                self.runtime.schedule_operation(&operation);
                0
            }
            StateTrivialityRequest::TestTrivialState(test)  => {
                let operation = StateTrivialityOperation::TestTrivialState(test);
                self.runtime.schedule_operation(&operation);
                0
            }
        }
    }
}
