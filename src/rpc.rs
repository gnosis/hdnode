//! Module for JSON RPC types.

use crate::serialization;
use serde::{
    de::{self, Deserializer, Visitor},
    Deserialize, Serialize,
};
use serde_json::{Map, Number, Value};
use std::fmt::{self, Formatter};

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
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Id {
    String(String),
    Number(Number),
    Null,
}

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct IdVisitor;

        impl<'de> Visitor<'de> for IdVisitor {
            type Value = Id;

            fn expecting(&self, f: &mut Formatter) -> fmt::Result {
                f.write_str("a JSON RPC id, either a number, string or null")
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Id::Number(v.into()))
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Id::Number(v.into()))
            }

            fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Id::Number(Number::from_f64(v).ok_or_else(|| {
                    de::Error::invalid_type(de::Unexpected::Float(v), &self)
                })?))
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_string(v.to_owned())
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Id::String(v))
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Id::Null)
            }
        }

        deserializer.deserialize_any(IdVisitor)
    }
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
    jsonrpc: JsonRpc,
    method: String,
    params: Option<Params>,
    id: Id,
}

/// JSON RPC response.
#[derive(Debug, Deserialize, Serialize)]
pub struct Response {
    jsonrpc: JsonRpc,
    #[serde(flatten, with = "serialization::result")]
    result: Result<Value, Error>,
    id: Id,
}

/// JSON RPC error.
#[derive(Debug, Deserialize, Serialize)]
pub struct Error {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
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
