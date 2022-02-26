//! Module implemeting the HD node handler.

use crate::{
    jsonrpc::{self, Id, JsonRpc, Params, Request, Response},
    serialization::{Addresses, Bytes},
    signer::{wallet::UnknownSignerError, BoxSigner},
};
use anyhow::Result;
use rocket::{
    serde::{
        json::{self, Json, Value},
        DeserializeOwned, Serialize,
    },
    State,
};

#[rocket::post("/", format = "json", data = "<request>")]
pub async fn request(request: Json<Request>, node: &State<Node>) -> Json<Response> {
    Json(node.handle_request(request.into_inner()).await)
}

#[rocket::post("/", format = "json", data = "<requests>", rank = 2)]
pub async fn batch(requests: Json<Vec<Request>>, node: &State<Node>) -> Json<Vec<Response>> {
    Json(node.handle_requests(requests.into_inner()).await)
}

#[rocket::post("/", data = "<data>", rank = 3)]
pub fn error(data: String) -> Json<Response> {
    // We get here if we are unable to parse the HTTP body as either a JSON-RPC
    // request or batch of requests. According to the spec, we need to either
    // error as a parse error (in case the JSON is invalid) or an invalid
    // request error (in case the JSON is valid, but not a valid Request).

    let err = if json::from_str::<Value>(&data).is_err() {
        tracing::debug!(%data, "HTTP body does not contain valid JSON");
        jsonrpc::Error::parse_error()
    } else {
        tracing::debug!(%data, "HTTP body is not a valid request or batch");
        jsonrpc::Error::invalid_request()
    };

    Json(Response {
        jsonrpc: JsonRpc::V2,
        result: Err(err),
        id: Id::Null,
    })
}

/// HD Node.
pub struct Node {
    signer: BoxSigner,
    remote: jsonrpc::Client,
}

impl Node {
    /// Creates a new HD node instance.
    pub fn new(signer: BoxSigner, remote: jsonrpc::Client) -> Self {
        Self { signer, remote }
    }

    /// Handles an RPC request.
    pub async fn handle_request(&self, request: Request) -> Response {
        match self.mux(request) {
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
        let (responses, remote_requests) = requests.into_iter().fold(
            (
                Vec::with_capacity(request_count),
                Vec::with_capacity(request_count),
            ),
            |(mut responses, mut remote), request| {
                match self.mux(request) {
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
    fn mux(&self, request: Request) -> Outcome {
        match self.mux_handler(&request.method, request.params.clone()) {
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
    fn mux_handler(&self, method: &str, params: Option<Params>) -> Result<Handled, jsonrpc::Error> {
        match &*method {
            "eth_accounts" => Handled::internal(params, |()| Ok(Addresses(self.signer.accounts()))),
            "eth_sendTransaction" => {
                let signed_transaction = self
                    .mux_handler("eth_signTransaction", params)?
                    .into_internal()
                    .expect("`eth_signTransaction` was not handled internally");
                Ok(Handled::Remote(
                    "eth_sendRawTransaction".to_owned(),
                    Some(Params::Array(vec![signed_transaction])),
                ))
            }
            "eth_sign" => Handled::internal(params, |(account, Bytes::<Vec<_>>(data))| {
                Ok(Bytes::from_signature(
                    self.signer.sign_message(account, &data)?,
                ))
            }),
            "eth_signTransaction" => Handled::internal(params, |(account, transaction)| {
                let signature = self.signer.sign_transaction(account, &transaction)?;
                Ok(Bytes(transaction.encode(signature)))
            }),
            "eth_signTypedData" => Handled::internal(params, |(account, typed_data)| {
                Ok(Bytes::from_signature(
                    self.signer.sign_typed_data(account, &typed_data)?,
                ))
            }),

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
    fn internal<T, U, F>(params: Option<Params>, f: F) -> Result<Self, jsonrpc::Error>
    where
        T: DeserializeOwned,
        U: Serialize,
        F: FnOnce(T) -> Result<U, jsonrpc::Error>,
    {
        let params = params.map(Value::from).unwrap_or(Value::Null);
        let params = T::deserialize(params).map_err(|err| {
            tracing::debug!(?err, "failed to deserialize parameters");
            jsonrpc::Error::invalid_params()
        })?;

        let value = f(params)?;
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
