// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use linera_sdk::views::{linera_views, CustomMapView, MapView, RootView, ViewStorageContext};
use social::{Key, OwnPost, Post};

/// The application state.
#[derive(RootView, async_graphql::SimpleObject)]
#[view(context = ViewStorageContext)]
pub struct SocialState {
    /// Our posts.
    pub own_posts: MapView<u32, OwnPost>,
    /// Posts we received from authors we subscribed to.
    pub received_posts: CustomMapView<Key, Post>,
}
