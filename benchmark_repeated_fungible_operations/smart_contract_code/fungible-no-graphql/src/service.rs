// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use std::sync::Arc;

use fungible_no_graphql::FungibleNoGraphQlTokenAbi;
use fungible_no_graphql::FungibleNoGraphQlRequest;
use fungible::{OwnerSpender, Parameters};
use linera_sdk::{
    linera_base_types::{Amount, WithServiceAbi},
    views::View,
    Service, ServiceRuntime,
};

use self::state::FungibleTokenState;

#[derive(Clone)]
pub struct FungibleTokenService {
    state: Arc<FungibleTokenState>,
    runtime: Arc<ServiceRuntime<Self>>,
}

linera_sdk::service!(FungibleTokenService);

impl WithServiceAbi for FungibleTokenService {
    type Abi = FungibleNoGraphQlTokenAbi;
}

impl Service for FungibleTokenService {
    type Parameters = Parameters;

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        let state = FungibleTokenState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        FungibleTokenService {
            state: Arc::new(state),
            runtime: Arc::new(runtime),
        }
    }

    async fn handle_query(&self, request: FungibleNoGraphQlRequest) -> Amount {
        match request {
            FungibleNoGraphQlRequest::Balance { owner } => {
                self.state.balance_or_default(&owner).await
            },
            FungibleNoGraphQlRequest::Allowance { owner, spender } => {
                let owner_spender = OwnerSpender::new(owner, spender);
                self.state
                    .allowances
                    .get(&owner_spender)
                    .await
                    .expect("Failure in the retrieval")
                    .unwrap_or_default()
            },
            FungibleNoGraphQlRequest::Operation { operation } => {
                self.runtime.schedule_operation(&operation);
                Amount::ZERO
            },
            FungibleNoGraphQlRequest::Operations { operations } => {
                for operation in operations {
                    self.runtime.schedule_operation(&operation);
                }
                Amount::ZERO
            },
        }
    }
}
