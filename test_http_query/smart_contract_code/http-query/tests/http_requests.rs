// Integration tests that perform real HTTP requests to external endpoints.

#![cfg(not(target_arch = "wasm32"))]

use http_query::{HttpQueryAbi, HttpQueryRequest};
use linera_sdk::{
    linera_base_types::ApplicationId,
    test::{ActiveChain, TestValidator},
};

const COINBASE_URL: &str = "https://api.exchange.coinbase.com/products/BTC-USD/trades";
const KRAKEN_URL: &str = "https://api.kraken.com/0/public/Trades?pair=XBTUSDT";

async fn setup_validator() -> (TestValidator, ApplicationId<HttpQueryAbi>, ActiveChain) {
    let (mut validator, application_id, chain) =
        TestValidator::with_current_application::<HttpQueryAbi, _, _>((), ()).await;

    validator
        .change_resource_control_policy(|policy| {
            policy
                .http_request_allow_list
                .insert("api.exchange.coinbase.com".to_owned());
            policy
                .http_request_allow_list
                .insert("api.kraken.com".to_owned());
        })
        .await;

    (validator, application_id, chain)
}

#[tokio::test]
async fn service_query_can_fetch_coinbase_trades() {
    let (_validator, application_id, chain) = setup_validator().await;

    let response = chain
        .query(
            application_id,
            HttpQueryRequest::HttpGet(COINBASE_URL.to_owned()),
        )
        .await
        .response;

    assert!(response > 0, "Expected non-empty response body length");
}

#[tokio::test]
async fn service_query_can_fetch_kraken_trades() {
    let (_validator, application_id, chain) = setup_validator().await;

    let response = chain
        .query(
            application_id,
            HttpQueryRequest::HttpGet(KRAKEN_URL.to_owned()),
        )
        .await
        .response;

    assert!(response > 0, "Expected non-empty response body length");
}
