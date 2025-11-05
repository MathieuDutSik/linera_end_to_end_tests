// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use counter_no_state::CounterNoStateAbi;
use linera_sdk::{
    linera_base_types::ApplicationId,
    views::{linera_views, RegisterView, RootView, ViewStorageContext},
};

/// The application state.
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct StateTrivialityState {
    pub value: RegisterView<Option<ApplicationId<CounterNoStateAbi>>>,
}
