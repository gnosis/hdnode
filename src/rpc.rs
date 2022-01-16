//! Module for JSON RPC types.

use crate::serialization;
use hyper::{client::HttpConnector, http::uri::Scheme, Uri};
use hyper_tls::HttpsConnector;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use std::sync::atomic::AtomicU64;

/// JSON RPC client.
pub struct Client {
    inner: Inner,
    uri: Uri,
}

impl Client {
    /// Creates a new client for the given URL.
    pub fn new(uri: Uri) -> Result<Self, ()> {
        let inner = match uri.scheme() {
            Some(s) if *s == Scheme::HTTP => Inner::Http(hyper::Client::new()),
            Some(s) if *s == Scheme::HTTPS => {
                let https = HttpsConnector::new();
                let client = hyper::Client::builder().build::<_, hyper::Body>(https);
                Inner::Https(client)
            }
            _ => {
                // TODO: error
                return Err(());
            }
        };

        Ok(Self { inner, uri })
    }

    /// Executes a JSON RPC request.
    pub async fn execute(&self, request: Request) -> Result<Response, ()> {
        self.inner.post(&self.uri, request).await
    }

    /// Executes a JSON RPC request batch.
    pub async fn execute_many(&self, requests: Vec<Request>) -> Result<Vec<Response>, ()> {
        self.inner.post(&self.uri, requests).await
    }
}

/// A `hyper` HTTP adapter to deal with different schemes.
enum Inner {
    Http(hyper::Client<HttpConnector>),
    Https(hyper::Client<HttpsConnector<HttpConnector>>),
}

impl Inner {
    /// Perform HTTP POST for the specified JSON data and parse JSON output.
    async fn post<T, U>(&self, uri: &Uri, data: T) -> Result<U, ()>
    where
        T: Serialize,
        U: DeserializeOwned,
    {
        let request = hyper::Request::post(uri)
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(&data).map_err(|_| ())?.into())
            .map_err(|_| ())?;

        let response = match self {
            Self::Http(client) => client.request(request),
            Self::Https(client) => client.request(request),
        }
        .await
        .map_err(|_| ())?;

        let (_parts, body) = response.into_parts();
        let bytes = hyper::body::to_bytes(body).await.map_err(|_| ())?;
        let result = serde_json::from_slice(&bytes).map_err(|_| ())?;

        Ok(result)
    }
}

/// JSON RPC version.
#[derive(Debug, Deserialize, Serialize)]
pub enum JsonRpc {
    #[serde(rename = "2.0")]
    V2,
}

/// JSON RPC message identifier.
///
/// From the specification:
/// > An identifier established by the Client that MUST contain a String,
/// > Number, or NULL value if included. If it is not included it is assumed to
/// > be a notification. The value SHOULD normally not be Null and Numbers
/// > SHOULD NOT contain fractional parts
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Id {
    String(String),
    Number(Number),
    Null,
}

/// JSON RPC params.
///
/// From the specification:
/// > If present, parameters for the rpc call MUST be provided as a structured
/// > value. Either by-position through an Array or by-name through an Object.
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Params {
    Array(Vec<Value>),
    Object(Map<String, Value>),
}

/// JSON RPC request.
#[derive(Debug, Deserialize, Serialize)]
pub struct Request {
    pub jsonrpc: JsonRpc,
    pub method: String,
    pub params: Option<Params>,
    pub id: Id,
}

/// JSON RPC response.
#[derive(Debug, Deserialize, Serialize)]
pub struct Response {
    pub jsonrpc: JsonRpc,
    #[serde(flatten, with = "serialization::result")]
    pub result: Result<Value, Error>,
    pub id: Id,
}

/// JSON RPC error.
#[derive(Debug, Deserialize, Serialize)]
pub struct Error {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn jsonrpc_version() {
        assert_eq!(serde_json::to_value(&JsonRpc::V2).unwrap(), json!("2.0"));
    }

    #[test]
    fn id() {
        assert_eq!(
            serde_json::to_value(&Id::String("1".to_string())).unwrap(),
            json!("1"),
        );
        assert_eq!(
            serde_json::to_string(&Id::Number(42_i32.into())).unwrap(),
            "42"
        );
        assert_eq!(
            serde_json::to_string(&Id::Number(Number::from_f64(13.37).unwrap())).unwrap(),
            "13.37"
        );
        assert_eq!(serde_json::to_value(&Id::Null).unwrap(), Value::Null);
    }

    #[test]
    fn invalid_missing_id() {
        assert!(serde_json::from_value::<Request>(json!({
            "jsonrpc": "2.0",
            "method": "foo",
            "params": [],
        }))
        .is_err());
    }

    #[test]
    fn request() {
        assert_eq!(
            serde_json::to_value(&Request {
                jsonrpc: JsonRpc::V2,
                method: "foo".to_string(),
                params: Some(Params::Array(vec![json!(1), json!("2")])),
                id: Id::Number(42.into()),
            })
            .unwrap(),
            json!({
                "jsonrpc": "2.0",
                "method": "foo",
                "params": (1, "2"),
                "id": 42,
            }),
        );
    }

    #[test]
    fn responses() {
        assert_eq!(
            serde_json::to_value(&Response {
                jsonrpc: JsonRpc::V2,
                result: Ok(json!("foo")),
                id: Id::Number(42.into()),
            })
            .unwrap(),
            json!({
                "jsonrpc": "2.0",
                "result": "foo",
                "id": 42,
            }),
        );
        assert_eq!(
            serde_json::to_value(&Response {
                jsonrpc: JsonRpc::V2,
                result: Err(Error {
                    code: 42,
                    message: "error".to_string(),
                    data: None,
                }),
                id: Id::Number(42.into()),
            })
            .unwrap(),
            json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": 42,
                    "message": "error",
                },
                "id": 42,
            }),
        );
    }
}
