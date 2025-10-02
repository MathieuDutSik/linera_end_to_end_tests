// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use linera_sdk::views::{linera_views, RegisterView, CollectionView, LogView, MapView, RootView, ViewStorageContext};

/// The application state.
#[derive(RootView, async_graphql::SimpleObject)]
#[view(context = ViewStorageContext)]
pub struct ComplexDataState {
    pub field1: RegisterView<u64>,
    pub field2: CollectionView<String, RegisterView<u8>>,
    pub field3: CollectionView<Vec<String>, LogView<u16>>,
    pub field4: CollectionView<String, MapView<String, u64>>,
}
