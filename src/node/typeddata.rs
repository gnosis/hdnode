//! Typed data for RPC calls.
//!
//! This is just a thin wrapper around `hdwallet::typeddata::TypedData` that
//! keeps track of its original JSON blob so it can debug and re-serialize it.

use rocket::serde::{
    de,
    json::{self, Value},
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::{
    fmt::{self, Debug, Formatter},
    ops::Deref,
};

/// Inner actual `TypedData` implementation.
type Inner = hdwallet::typeddata::TypedData;

/// A wrapper type around `hdwallet::typeddata::TypedData` that implements JSON
/// serialization and debug printing.
pub struct TypedData {
    raw: Value,
    inner: Inner,
}

impl Deref for TypedData {
    type Target = Inner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Debug for TypedData {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

impl Serialize for TypedData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.raw.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TypedData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = Value::deserialize(deserializer)?;
        let inner = json::from_value(raw.clone()).map_err(de::Error::custom)?;
        Ok(Self { raw, inner })
    }
}
