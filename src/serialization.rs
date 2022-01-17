//! Module containing serialization helpers.

use std::borrow::Cow;

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

/// Hex-encoded bytes serializer.
pub struct Bytes<T>(pub T);

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
