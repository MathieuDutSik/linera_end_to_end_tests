// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use std::sync::Arc;

use async_graphql::{EmptySubscription, Request, Response, Schema};
use reported_solutions::{ReportedSolutionsAbi, ReportedSolutionsOperation};
use linera_sdk::{linera_base_types::WithServiceAbi, views::View, Service, ServiceRuntime};
use linera_sdk::graphql::{GraphQLMutationRoot as _};



use self::state::ReportedSolutionsState;

pub struct ReportedSolutionsService {
    state: Arc<ReportedSolutionsState>,
    runtime: Arc<ServiceRuntime<Self>>,
}

linera_sdk::service!(ReportedSolutionsService);

impl WithServiceAbi for ReportedSolutionsService {
    type Abi = ReportedSolutionsAbi;
}

impl Service for ReportedSolutionsService {
    type Parameters = ();

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        let state = ReportedSolutionsState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        ReportedSolutionsService {
            state: Arc::new(state),
            runtime: Arc::new(runtime),
        }
    }

    async fn handle_query(&self, request: Request) -> Response {
        let schema = Schema::build(
            self.state.clone(),
            ReportedSolutionsOperation::mutation_root(self.runtime.clone()),
            EmptySubscription,
        )
            .finish();
        schema.execute(request).await
    }
}
