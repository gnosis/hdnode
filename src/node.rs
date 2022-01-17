//! Module implemeting the HD node handler.

mod method;
mod remote;

use self::{
    method::{eth, Method},
    remote::Remote,
};
use crate::{
    jsonrpc::{self, InvalidScheme, Request, Response},
    serialization::Bytes,
};
use hdwallet::account::{Address, PrivateKey};
use hyper::Uri;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// HD Node.
pub struct Node {
    accounts: Vec<PrivateKey>,
    remote: Remote,
    address_map: HashMap<[u8; 20], usize>,
}

impl Node {
    /// Creates a new HD node instance.
    pub fn new(accounts: Vec<PrivateKey>, remote_uri: Uri) -> Result<Self, InvalidScheme> {
        let remote = Remote::new(remote_uri)?;
        let address_map = accounts
            .iter()
            .enumerate()
            .map(|(index, account)| (account.address().0, index))
            .collect();

        Ok(Self {
            accounts,
            remote,
            address_map,
        })
    }

    /// Retrieves the private key for the specified address.
    fn account(&self, address: Address) -> Option<&PrivateKey> {
        let index = self.address_map.get(&address.0)?;
        Some(&self.accounts[*index])
    }

    /*
    /// Handles an RPC request.
    pub async fn handle_request(&self, request: Request) -> Response {}

    /// Handles an RPC request.
    pub async fn handle_batch(&self, requests: Vec<Request>) -> Vec<Response> {}
    */

    /// Attempts to handles an RPC request locally.
    ///
    /// This method either returns the response if the RPC was fully handled, or
    /// a request to be forwarded to the remote node.
    fn local(&self, request: Request) -> Local {
        match request.method.as_str() {
            s if &eth::Sign == s => {
                Local::Internal(handler::<eth::Sign, _>(request, |(address, message)| {
                    todo!()
                }))
            }
            _ => Local::Remote(request),
        }
    }
}

/// Internal result of handling an RPC request locally.
enum Local {
    /// Request was handled internally my the node.
    Internal(Response),

    /// Request was either partially handled or not handled at all by the node.
    /// The specified request must be forwarded to the remote.
    Remote(Request),
}

/// Helper method for implementing handlers for typed RPC methods.
fn handler<M, F>(request: Request, f: F) -> Response
where
    M: Method,
    M::Params: DeserializeOwned,
    M::Result: Serialize,
    F: FnOnce(M::Params) -> Result<M::Result, jsonrpc::Error>,
{
    let params = request.params.map(Value::from).unwrap_or_default();
    let result = serde_json::from_value(params)
        .map_err(|_| jsonrpc::Error::invalid_params())
        .and_then(f)
        .and_then(|result| {
            serde_json::to_value(&result).map_err(|_| jsonrpc::Error::internal_error())
        });

    Response {
        jsonrpc: request.jsonrpc,
        result,
        id: request.id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calls() {}
}
