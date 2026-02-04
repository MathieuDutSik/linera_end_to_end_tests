use linera_sdk::views::{linera_views, RegisterView, RootView, ViewStorageContext};

/// The application state.
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct HttpQueryState {
    pub last_response_length: RegisterView<u64>,
}
