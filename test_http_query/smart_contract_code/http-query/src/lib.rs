use linera_sdk::linera_base_types::{ContractAbi, ServiceAbi};
use serde::{Deserialize, Serialize};

pub struct HttpQueryAbi;

impl ContractAbi for HttpQueryAbi {
    type Operation = HttpQueryOperation;
    type Response = u64;
}

impl ServiceAbi for HttpQueryAbi {
    type Query = HttpQueryRequest;
    type QueryResponse = u64;
}

/// Requests that can be made to the service.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum HttpQueryRequest {
    /// Perform an HTTP GET to the given URL and return the response body length.
    HttpGet(String),
}

/// Operations that can be executed by the contract.
#[derive(Debug, Serialize, Deserialize)]
pub enum HttpQueryOperation {
    /// Perform an HTTP GET to the given URL and return the response body length.
    HttpGet(String),
}
