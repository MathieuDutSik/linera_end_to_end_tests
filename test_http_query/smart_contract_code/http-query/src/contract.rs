#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use http_query::{HttpQueryAbi, HttpQueryOperation};
use linera_sdk::{
    http,
    linera_base_types::WithContractAbi,
    views::{RootView, View},
    Contract, ContractRuntime,
};

use self::state::HttpQueryState;

pub struct HttpQueryContract {
    state: HttpQueryState,
    runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(HttpQueryContract);

impl WithContractAbi for HttpQueryContract {
    type Abi = HttpQueryAbi;
}

impl Contract for HttpQueryContract {
    type Message = ();
    type InstantiationArgument = ();
    type Parameters = ();
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = HttpQueryState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        HttpQueryContract { state, runtime }
    }

    async fn instantiate(&mut self, _value: ()) {}

    async fn execute_operation(&mut self, operation: HttpQueryOperation) -> u64 {
        match operation {
            HttpQueryOperation::HttpGet(url) => {
                let request = http::Request::get(&url);
                let response = self.runtime.http_request(request);
                let length = response.body.len() as u64;
                self.state.last_response_length.set(length);
                length
            }
        }
    }

    async fn execute_message(&mut self, _message: ()) {
        panic!("HttpQuery application doesn't support any cross-chain messages");
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}
