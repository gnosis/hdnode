//! A module implementing JSON RPC client for a remote Ethereum node.

use super::method::Method;
use crate::jsonrpc::{self, Client, ClientError, Id, InvalidScheme, JsonRpc, Params, Request};
use hyper::Uri;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::{
    fmt::{self, Display, Formatter},
    sync::atomic::{AtomicU64, Ordering},
};

/// A remote Ethereum node.
pub struct Remote {
    client: Client,
    ids: AtomicU64,
}

impl Remote {
    /// Create a new remote Ethereum node client.
    pub fn new(uri: Uri) -> Result<Self, InvalidScheme> {
        let client = Client::new(uri)?;
        Ok(Self {
            client,
            ids: Default::default(),
        })
    }

    /// Executes a typed method call.
    pub async fn execute<M>(&self, method: M, params: M::Params) -> Result<M::Result, RemoteError>
    where
        M: Method,
        M::Params: Serialize,
        M::Result: DeserializeOwned,
    {
        let params = match serde_json::to_value(params)? {
            Value::Array(array) => Params::Array(array),
            // Automatically promote single values to arrays with one entry. We
            // can do this for Ethereum JSON RPC requests since they MUST use
            // array parameters.
            value => Params::Array(vec![value]),
        };

        let id = self.ids.fetch_add(1, Ordering::Relaxed);
        let request = Request {
            jsonrpc: JsonRpc::V2,
            method: method.into_name(),
            params: Some(params),
            id: Id::Number(id.into()),
        };

        let response = self.client.execute(request).await?;
        let result = serde_json::from_value(response.result?)?;

        Ok(result)
    }
}

/// An error executing a JSON RPC request with a remote node.
#[derive(Debug)]
pub enum RemoteError {
    /// An error occured during JSON serialization of method parameters or
    /// result.
    Json(serde_json::Error),

    /// An error occured in the underlying JSON RPC client.
    Client(ClientError),

    /// A JSON RPC error occured while processing the request.
    Rpc(jsonrpc::Error),
}

impl Display for RemoteError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::Json(err) => write!(f, "JSON serialization error: {err}"),
            Self::Client(err) => write!(f, "client error: {err}"),
            Self::Rpc(err) => write!(f, "Ethereum RPC error: {err}"),
        }
    }
}

impl From<serde_json::Error> for RemoteError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}

impl From<ClientError> for RemoteError {
    fn from(err: ClientError) -> Self {
        Self::Client(err)
    }
}

impl From<jsonrpc::Error> for RemoteError {
    fn from(err: jsonrpc::Error) -> Self {
        Self::Rpc(err)
    }
}
