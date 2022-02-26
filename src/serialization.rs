//! Module containing serialization helpers.

use hdwallet::account::{Address, Signature};
use rocket::serde::{
    de,
    json::{self, Value},
    ser::SerializeSeq,
    Deserialize, DeserializeOwned, Deserializer, Serialize, Serializer,
};
use std::{
    borrow::Cow,
    fmt::{self, Debug, Formatter},
    ops::Deref,
};

/// Type repesenting empty JSON RPC parameters.
pub type NoParameters = [(); 0];

/// Hex-encoded bytes serializer.
pub struct Bytes<T>(pub T);

impl Bytes<[u8; 65]> {
    /// Helper method to convert a `Signature` to raw bytes.
    pub fn from_signature(signature: Signature) -> Self {
        let mut buffer = [0_u8; 65];
        buffer[..32].copy_from_slice(&signature.r);
        buffer[32..64].copy_from_slice(&signature.s);
        buffer[64] = signature.v();
        Bytes(buffer)
    }
}

impl Serialize for Bytes<Vec<u8>> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("0x{}", hex::encode(&self.0)))
    }
}

impl<const N: usize> Serialize for Bytes<[u8; N]> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("0x{}", hex::encode(&self.0)))
    }
}

impl<'de> Deserialize<'de> for Bytes<Vec<u8>> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = Cow::<str>::deserialize(deserializer)?;
        let s = s
            .strip_prefix("0x")
            .ok_or_else(|| de::Error::custom("storage slot missing '0x' prefix"))?;
        hex::decode(s).map(Bytes).map_err(de::Error::custom)
    }
}

impl<'de, const N: usize> Deserialize<'de> for Bytes<[u8; N]> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = Cow::<str>::deserialize(deserializer)?;
        let s = s
            .strip_prefix("0x")
            .ok_or_else(|| de::Error::custom("storage slot missing '0x' prefix"))?;

        let mut b = [0_u8; N];
        hex::decode_to_slice(s, &mut b).map_err(de::Error::custom)?;

        Ok(Bytes(b))
    }
}

/// Serialization and debug implementation for a slice of addresses.
pub struct Addresses<'a>(pub &'a [Address]);

impl Debug for Addresses<'_> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        struct DisplayAsDebug<'b>(&'b Address);
        impl Debug for DisplayAsDebug<'_> {
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        f.debug_list()
            .entries(self.0.iter().map(DisplayAsDebug))
            .finish()
    }
}

impl Serialize for Addresses<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for address in self.0 {
            seq.serialize_element(&address.to_string())?;
        }
        seq.end()
    }
}

/// A wrapper type around `Deserialize` implementations that keeps the original
/// JSON object for debug printing.
pub struct Raw<T> {
    raw: Value,
    inner: T,
}

impl<T> Deref for Raw<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> Debug for Raw<T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

impl<'de, T> Deserialize<'de> for Raw<T>
where
    T: DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = Value::deserialize(deserializer)?;
        let inner = json::from_value(raw.clone()).map_err(de::Error::custom)?;
        Ok(Self { raw, inner })
    }
}

/// Module implementing serialization for types that implement standard string
/// conversion methods.
pub mod str {
    use rocket::serde::{de, Deserialize as _, Deserializer};
    use std::{borrow::Cow, fmt::Display, str::FromStr};

    #[doc(hidden)]
    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        T: FromStr,
        T::Err: Display,
        D: Deserializer<'de>,
    {
        let s = Cow::<str>::deserialize(deserializer)?;
        T::from_str(&*s).map_err(de::Error::custom)
    }
}
