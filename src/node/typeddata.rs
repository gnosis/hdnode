//! Typed data for RPC calls.
//!
//! This is just a thin wrapper around `hdwallet::typeddata::TypedData` that
//! keeps track of its original JSON blob so it can debug and re-serialize it.

use crate::node::eth::Eth;
use anyhow::{ensure, Result};
use ethnum::U256;
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
    chain_id: Option<U256>,
}

impl TypedData {
    /// Verifies the typed data domain is compatible with the connected node.
    pub async fn verify(&self, eth: &Eth) -> Result<()> {
        if let Some(chain_id) = self.chain_id {
            ensure!(
                chain_id == eth.chain_id().await?,
                "chain ID used for signing does not match node",
            );
        }

        Ok(())
    }
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

        // Be extra permissive with `chainId` because EIP-712 doesn't really
        // standardize its representation.
        let chain_id = match &raw["domain"]["chainId"] {
            Value::Null => None,
            Value::Number(value) if value.is_u64() => value.as_u64().map(U256::from),
            Value::String(value) => {
                let (s, radix) = match value.strip_prefix("0x") {
                    Some(s) => (s, 16),
                    None => (&**value, 10),
                };
                Some(U256::from_str_radix(s, radix).map_err(de::Error::custom)?)
            }
            other => {
                return Err(de::Error::custom(format!(
                    "invalid chain ID value in domain '{other}'"
                )))
            }
        };

        Ok(Self {
            raw,
            inner,
            chain_id,
        })
    }
}
