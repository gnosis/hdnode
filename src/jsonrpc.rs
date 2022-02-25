//! Module for JSON RPC types.

use crate::VERSION;
use anyhow::{bail, ensure, Context as _, Result};
use reqwest::Url;
use rocket::serde::{
    de::{self, DeserializeOwned},
    json::{
        self,
        serde_json::{Map, Number},
        Value,
    },
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::{
    borrow::Cow,
    fmt::{self, Display, Formatter},
};

/// JSON RPC client.
pub struct Client {
    client: reqwest::Client,
    url: Url,
}

impl Client {
    /// Creates a new client for the given URL.
    pub fn new(url: Url) -> Result<Self> {
        let client = reqwest::Client::builder().user_agent(VERSION).build()?;
        Ok(Self { client, url })
    }

    /// Returns the URL of the current RPC client.
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Executes a JSON RPC request.
    pub async fn execute(&self, request: &Request) -> Result<Response> {
        self.post(request).await
    }

    /// Executes a JSON RPC request batch.
    pub async fn execute_many(&self, requests: &[Request]) -> Result<Vec<Response>> {
        if requests.is_empty() {
            return Ok(Vec::new());
        }

        let responses = self.post::<_, Vec<Response>>(requests).await?;

        if requests.len() != responses.len()
            || requests
                .iter()
                .zip(responses.iter())
                .all(|(request, response)| request.id == response.id)
        {
            tracing::error!(
                ?requests,
                ?responses,
                "mismatched batched requests and responses",
            );
            bail!("mismatched batched requests and responses");
        }

        Ok(responses)
    }

    /// Perform HTTP POST for the specified JSON data and parse JSON output.
    async fn post<T, U>(&self, data: T) -> Result<U>
    where
        T: Serialize,
        U: DeserializeOwned,
    {
        let response = self
            .client
            .post(self.url.clone())
            .json(&data)
            .send()
            .await
            .context("failed to send request")?;

        let status = response.status();
        let text = response
            .text()
            .await
            .context("failed to read response body")?;

        ensure!(status.is_success(), "HTTP {status} error: {text}");
        json::from_str(&text)
            .with_context(|| format!("response body: {text}"))
            .context("failed to parse response as JSON")
    }
}

/// JSON RPC version.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
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
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(crate = "rocket::serde", untagged)]
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
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(crate = "rocket::serde", untagged)]
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
#[serde(crate = "rocket::serde")]
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
#[serde(crate = "rocket::serde")]
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
#[serde(crate = "rocket::serde")]
pub struct Error {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl Error {
    /// Creates an error indicating the provided JSON was not a valid request.
    pub fn parse_error() -> Self {
        Self {
            code: -32700,
            message: "Parse error".to_owned(),
            data: None,
        }
    }

    /// Creates an error indicating the provided JSON was not a valid request.
    pub fn invalid_request() -> Self {
        Self {
            code: -32600,
            message: "Invalid Request".to_owned(),
            data: None,
        }
    }

    /// Creates an error indicating parameters were invalid.
    pub fn invalid_params() -> Self {
        Self {
            code: -32602,
            message: "Invalid params".to_owned(),
            data: None,
        }
    }

    /// Creates an error indicating an internal server error was encountered.
    pub fn internal_error() -> Self {
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
    use rocket::serde::json::serde_json::{self, json};

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
