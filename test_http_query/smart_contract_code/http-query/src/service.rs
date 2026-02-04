#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use std::sync::Arc;

use http_query::{HttpQueryAbi, HttpQueryRequest};
use linera_sdk::{
    http,
    linera_base_types::WithServiceAbi,
    views::View,
    Service, ServiceRuntime,
};

use self::state::HttpQueryState;

pub struct HttpQueryService {
    _state: HttpQueryState,
    runtime: Arc<ServiceRuntime<Self>>,
}

linera_sdk::service!(HttpQueryService);

impl WithServiceAbi for HttpQueryService {
    type Abi = HttpQueryAbi;
}

impl Service for HttpQueryService {
    type Parameters = ();

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        let state = HttpQueryState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        HttpQueryService {
            _state: state,
            runtime: Arc::new(runtime),
        }
    }

    async fn handle_query(&self, request: HttpQueryRequest) -> u64 {
        match request {
            HttpQueryRequest::HttpGet(url) => {
                let request = http::Request::get(&url);
                let response = self.runtime.http_request(request);
                response.body.len() as u64
            }
        }
    }
}
