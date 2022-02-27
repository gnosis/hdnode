//! Module implemeting the HD node handler.

pub mod eth;
pub mod transaction;
pub mod typeddata;
pub mod types;

use self::{eth::Eth, transaction::TransactionRequest};
use crate::{
    jsonrpc::{self, Id, JsonRpc, Params, Request, Response},
    serialization::{Addresses, Bytes, NoParameters},
    signer::{wallet::UnknownSignerError, BoxSigner},
};
use anyhow::Result;
use rocket::{
    futures::future,
    serde::{
        json::{self, Json, Value},
        Deserialize, DeserializeOwned, Serialize,
    },
    State,
};
use std::future::Future;

/// Helper type with different handler input types.
///
/// This is needed to work around the fact that Rocket shortcuts the forwarding
/// process if it fails to deserialize its input.
#[derive(Deserialize)]
#[serde(crate = "rocket::serde", untagged)]
pub enum Input {
    Request(Request),
    Batch(Vec<Request>),
    Unrecognized(Value),
}

/// Helper type with different handler output types.
#[derive(Serialize)]
#[serde(crate = "rocket::serde", untagged)]
pub enum Output {
    Response(Response),
    Batch(Vec<Response>),
}

#[rocket::post("/", format = "json", data = "<input>")]
pub async fn handler(input: Json<Input>, node: &State<Node>) -> Json<Output> {
    let output = match input.into_inner() {
        Input::Request(request) => Output::Response(node.handle_request(request).await),
        Input::Batch(requests) => Output::Batch(node.handle_requests(requests).await),
        Input::Unrecognized(data) => {
            tracing::debug!(%data, "HTTP body is not a valid request or batch");
            Output::Response(Response {
                jsonrpc: JsonRpc::V2,
                result: Err(jsonrpc::Error::invalid_request()),
                id: Id::Null,
            })
        }
    };
    Json(output)
}

/// HD Node.
pub struct Node {
    signer: BoxSigner,
    remote: Eth,
}

impl Node {
    /// Creates a new HD node instance.
    pub fn new(signer: BoxSigner, remote: Eth) -> Self {
        Self { signer, remote }
    }

    /// Handles an RPC request.
    pub async fn handle_request(&self, request: Request) -> Response {
        match self.mux(request).await {
            Outcome::Internal(response) => response,
            Outcome::Remote(request) => match self.remote.execute(&request).await {
                Ok(response) => response,
                Err(err) => {
                    tracing::debug!(?err, ?request, "error executing remote request");
                    Response {
                        jsonrpc: request.jsonrpc,
                        result: Err(jsonrpc::Error::internal_error()),
                        id: request.id,
                    }
                }
            },
        }
    }

    /// Handles an RPC batch.
    pub async fn handle_requests(&self, requests: Vec<Request>) -> Vec<Response> {
        let request_count = requests.len();
        let outcomes =
            future::join_all(requests.into_iter().map(|request| self.mux(request))).await;
        let (responses, remote_requests) = outcomes.into_iter().fold(
            (
                Vec::with_capacity(request_count),
                Vec::with_capacity(request_count),
            ),
            |(mut responses, mut remote), outcome| {
                match outcome {
                    Outcome::Internal(response) => responses.push(Some(response)),
                    Outcome::Remote(request) => {
                        responses.push(None);
                        remote.push(request);
                    }
                }
                (responses, remote)
            },
        );

        let remote_responses = match self.remote.execute_many(&remote_requests).await {
            Ok(responses) => responses,
            Err(err) => {
                tracing::debug!(
                    ?err,
                    ?remote_requests,
                    "error executing remote batched requests"
                );
                remote_requests
                    .into_iter()
                    .map(|request| Response {
                        jsonrpc: request.jsonrpc,
                        result: Err(jsonrpc::Error::internal_error()),
                        id: request.id,
                    })
                    .collect::<Vec<_>>()
            }
        };

        let mut remote_responses = remote_responses.into_iter();
        let responses = responses
            .into_iter()
            .map(|response| {
                response
                    .or_else(|| remote_responses.next())
                    .expect("no internal or remote response")
            })
            .collect();
        debug_assert!(
            remote_responses.next().is_none(),
            "leftover remote response"
        );

        responses
    }

    /// Takes a single request and either handles it internally or producing a
    /// response or returns another request to be sent to the remote node.
    ///
    /// This allows requests to either be completely proxied to the remote node
    /// or partially handled internally.
    async fn mux(&self, request: Request) -> Outcome {
        match self
            .mux_handler(&request.method, request.params.clone())
            .await
        {
            Ok(Handled::Internal(value)) => Outcome::Internal(Response {
                jsonrpc: request.jsonrpc,
                result: Ok(value),
                id: request.id,
            }),
            Ok(Handled::Remote(method, params)) => Outcome::Remote(Request {
                jsonrpc: request.jsonrpc,
                method,
                params,
                id: request.id,
            }),
            Err(err) => {
                tracing::debug!(?request, "error processing request");
                Outcome::Internal(Response {
                    jsonrpc: request.jsonrpc,
                    result: Err(err),
                    id: request.id,
                })
            }
        }
    }

    /// Handler method for a particular request method and parameters.
    async fn mux_handler(
        &self,
        method: &str,
        params: Option<Params>,
    ) -> Result<Handled, jsonrpc::Error> {
        match method {
            "eth_accounts" => {
                Handled::internal(params, |_: NoParameters| async {
                    Ok(Addresses(self.signer.accounts()))
                })
                .await
            }
            "eth_sendTransaction" | "eth_signTransaction" => {
                let signed_transaction =
                    Handled::internal(params, |(transaction,): (TransactionRequest,)| async {
                        let (account, transaction) = transaction.fill(&self.remote).await?;
                        let signature = self.signer.sign_transaction(account, &transaction)?;
                        Ok(Bytes(transaction.encode(signature)))
                    })
                    .await?;

                if method == "eth_sendTransaction" {
                    Ok(Handled::Remote(
                        "eth_sendRawTransaction".to_owned(),
                        Some(Params::Array(vec![signed_transaction
                            .into_internal()
                            .unwrap()])),
                    ))
                } else {
                    Ok(signed_transaction)
                }
            }
            "eth_sign" => {
                Handled::internal(params, |(account, data): (_, Bytes<Vec<_>>)| async move {
                    Ok(Bytes::from_signature(
                        self.signer.sign_message(account, &data)?,
                    ))
                })
                .await
            }
            "eth_signTypedData" => {
                Handled::internal(params, |(account, typed_data)| async move {
                    Ok(Bytes::from_signature(
                        self.signer.sign_typed_data(account, &typed_data)?,
                    ))
                })
                .await
            }

            _ => Ok(Handled::Remote(method.to_owned(), params)),
        }
    }
}

/// Internal outcome of handling an RPC request locally.
enum Outcome {
    /// Request was handled internally my the node.
    Internal(Response),

    /// Request was either partially handled or not handled at all by the node.
    /// The specified request must be forwarded to the remote.
    Remote(Request),
}

/// Internal intermediate result from handling a request.
enum Handled {
    /// Request was handled internally my the node and produced the following
    /// result value.
    Internal(Value),

    /// Request was either partially handled or not handled at all by the node.
    /// The specified request method and parameters must be forwarded to the
    /// remote.
    Remote(String, Option<Params>),
}

impl Handled {
    /// Creates a response to an internally handled request.
    async fn internal<T, U, F, Fut>(params: Option<Params>, f: F) -> Result<Self, jsonrpc::Error>
    where
        T: DeserializeOwned,
        U: Serialize,
        F: FnOnce(T) -> Fut,
        Fut: Future<Output = Result<U, jsonrpc::Error>>,
    {
        let params = params.map(Value::from).unwrap_or(Value::Null);
        let params = T::deserialize(params).map_err(|err| {
            tracing::debug!(?err, "failed to deserialize parameters");
            jsonrpc::Error::invalid_params()
        })?;

        let value = f(params).await?;
        let value = json::serde_json::to_value(&value).map_err(|err| {
            tracing::error!(?err, "unexpected error serializing response JSON");
            jsonrpc::Error::internal_error()
        })?;

        Ok(Self::Internal(value))
    }

    /// Returns the result value if it was handled internally or `None`
    /// otherwise.
    fn into_internal(self) -> Option<Value> {
        match self {
            Self::Internal(value) => Some(value),
            _ => None,
        }
    }
}

impl From<anyhow::Error> for jsonrpc::Error {
    fn from(err: anyhow::Error) -> Self {
        tracing::debug!(%err, "encountered error");
        if err.downcast_ref::<UnknownSignerError>().is_some() {
            jsonrpc::Error::invalid_params()
        } else {
            jsonrpc::Error::internal_error()
        }
    }
}
