//! Module for JSON RPC types.

use hyper::{client::HttpConnector, http::uri::Scheme, Uri};
use hyper_tls::HttpsConnector;
use serde::{
    de::{self, DeserializeOwned},
    Deserialize, Deserializer, Serialize, Serializer,
};
use serde_json::{Map, Number, Value};
use std::{
    borrow::Cow,
    fmt::{self, Display, Formatter},
};

/// JSON RPC client.
pub struct Client {
    inner: Inner,
    uri: Uri,
}

impl Client {
    /// Creates a new client for the given URL.
    pub fn new(uri: Uri) -> Result<Self, InvalidScheme> {
        let inner = match uri.scheme() {
            Some(s) if *s == Scheme::HTTP => Inner::Http(hyper::Client::new()),
            Some(s) if *s == Scheme::HTTPS => {
                let https = HttpsConnector::new();
                let client = hyper::Client::builder().build::<_, hyper::Body>(https);
                Inner::Https(client)
            }
            other => return Err(InvalidScheme(other.cloned())),
        };

        Ok(Self { inner, uri })
    }

    /// Executes a JSON RPC request.
    pub async fn execute(&self, request: Request) -> Result<Response, ClientError> {
        self.inner.post(&self.uri, request).await
    }

    /// Executes a JSON RPC request batch.
    pub async fn execute_many(&self, requests: Vec<Request>) -> Result<Vec<Response>, ClientError> {
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
    async fn post<T, U>(&self, uri: &Uri, data: T) -> Result<U, ClientError>
    where
        T: Serialize,
        U: DeserializeOwned,
    {
        let request = hyper::Request::post(uri)
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(&data)?.into())?;

        let response = match self {
            Self::Http(client) => client.request(request),
            Self::Https(client) => client.request(request),
        }
        .await?;

        let (_parts, body) = response.into_parts();
        let bytes = hyper::body::to_bytes(body).await?;
        let result = serde_json::from_slice(&bytes)?;

        Ok(result)
    }
}

/// Invalid URI scheme.
#[derive(Debug)]
pub struct InvalidScheme(pub Option<Scheme>);

impl Display for InvalidScheme {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match &self.0 {
            Some(s) => write!(f, "invalid scheme {s}"),
            None => f.write_str("missing scheme"),
        }
    }
}

impl std::error::Error for InvalidScheme {}

/// JSON RPC client error.
#[derive(Debug)]
pub enum ClientError {
    /// An error occured while preparing and HTTP request.
    Request(hyper::http::Error),

    /// An error occured while performing an HTTP request.
    Http(hyper::Error),

    /// An error occured serializing or deserializing JSON RPC data.
    Json(serde_json::Error),
}

impl Display for ClientError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::Request(err) => write!(f, "HTTP request error: {err}"),
            Self::Http(err) => write!(f, "HTTP error: {err}"),
            Self::Json(err) => write!(f, "JSON error: {err}"),
        }
    }
}

impl std::error::Error for ClientError {}

impl From<hyper::http::Error> for ClientError {
    fn from(err: hyper::http::Error) -> Self {
        Self::Request(err)
    }
}

impl From<hyper::Error> for ClientError {
    fn from(err: hyper::Error) -> Self {
        Self::Http(err)
    }
}

impl From<serde_json::Error> for ClientError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}

/// JSON RPC version.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
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
#[derive(Clone, Debug, Deserialize, Serialize)]
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

impl From<Params> for Value {
    fn from(val: Params) -> Self {
        match val {
            Params::Array(a) => Value::Array(a),
            Params::Object(o) => Value::Object(o),
        }
    }
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
#[derive(Debug)]
pub struct Response {
    pub jsonrpc: JsonRpc,
    pub result: Result<Value, Error>,
    pub id: Id,
}

/// Helper type for generating serialization implemtation for `Response`.
#[derive(Deserialize, Serialize)]
struct Res<'a> {
    jsonrpc: JsonRpc,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Cow<'a, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<Cow<'a, Error>>,
    id: Cow<'a, Id>,
}

impl Serialize for Response {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let (result, error) = match &self.result {
            Ok(result) => (Some(Cow::Borrowed(result)), None),
            Err(error) => (None, Some(Cow::Borrowed(error))),
        };
        let res = Res {
            jsonrpc: self.jsonrpc,
            result,
            error,
            id: Cow::Borrowed(&self.id),
        };
        res.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Response {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let res = Res::deserialize(deserializer)?;
        let result = match (res.result, res.error) {
            (Some(result), None) => Ok(result.into_owned()),
            (None, Some(error)) => Err(error.into_owned()),
            (Some(_), Some(_)) => return Err(de::Error::custom("both result and error specified")),
            (None, None) => return Err(de::Error::custom("missing result or error")),
        };
        Ok(Response {
            jsonrpc: res.jsonrpc,
            result,
            id: res.id.into_owned(),
        })
    }
}

/// JSON RPC error.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Error {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl Error {
    /// Creates an error indicating parameters were invalid.
    pub fn invalid_params() -> Error {
        Self {
            code: -32602,
            message: "Invalid params".to_owned(),
            data: None,
        }
    }

    /// Creates an error indicating an internal server error was encountered.
    pub fn internal_error() -> Error {
        Self {
            code: -32603,
            message: "Internal error".to_owned(),
            data: None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

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
