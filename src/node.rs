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
    wallet::Wallet,
};
use hdwallet::account::Signature;
use hyper::Uri;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

/// HD Node.
pub struct Node {
    wallet: Wallet,
    remote: Remote,
}

impl Node {
    /// Creates a new HD node instance.
    pub fn new(wallet: Wallet, remote_uri: Uri) -> Result<Self, InvalidScheme> {
        let remote = Remote::new(remote_uri)?;
        Ok(Self { wallet, remote })
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
            "eth_sign" => Local::Internal(handler(
                request,
                |(account, message): (_, Bytes<Vec<_>>)| {
                    self.wallet
                        .sign_message(account, &message.0)
                        .map(sig)
                        .map_err(jsonrpc::Error::todo)
                },
            )),
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
fn handler<T, U, E, F>(request: Request, f: F) -> Response
where
    T: DeserializeOwned,
    U: Serialize,
    E: Into<jsonrpc::Error>,
    F: FnOnce(T) -> Result<U, E>,
{
    let params = request.params.map(Value::from).unwrap_or_default();
    let result = serde_json::from_value(params)
        .map_err(|_| jsonrpc::Error::invalid_params())
        .and_then(|params| f(params).map_err(E::into))
        .and_then(|result| {
            serde_json::to_value(&result).map_err(|_| jsonrpc::Error::internal_error())
        });

    Response {
        jsonrpc: request.jsonrpc,
        result,
        id: request.id,
    }
}

/// Helper method to convert a `Signature` to raw bytes.
fn sig(signature: Signature) -> Bytes<[u8; 65]> {
    let mut buffer = [0_u8; 65];
    buffer[..32].copy_from_slice(&signature.r);
    buffer[32..64].copy_from_slice(&signature.s);
    buffer[64] = signature.v();
    Bytes(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calls() {}
}
