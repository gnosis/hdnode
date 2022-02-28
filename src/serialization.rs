//! Module containing serialization helpers.

use ethnum::U256;
use hdwallet::account::{Address, Signature};
use rocket::serde::{de, ser::SerializeSeq as _, Deserialize, Deserializer, Serialize, Serializer};
use std::{
    borrow::Cow,
    fmt::{self, Debug, Display, Formatter},
    ops::Deref,
    str::FromStr,
};

/// Type repesenting empty JSON RPC parameters.
pub type NoParameters = [(); 0];

/// Hex-encoded bytes serializer.
#[derive(Clone, Default)]
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

impl<T> Debug for Bytes<T>
where
    Bytes<T>: Serialize,
{
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        self.serialize(f)
    }
}

impl<T> Deref for Bytes<T>
where
    T: AsRef<[u8]>,
{
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl Serialize for Bytes<&'_ [u8]> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("0x{}", hex::encode(self.0)))
    }
}

impl Serialize for Bytes<Vec<u8>> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Bytes(&self.0[..]).serialize(serializer)
    }
}

impl<const N: usize> Serialize for Bytes<[u8; N]> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Bytes(&self.0[..]).serialize(serializer)
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

/// Module implementing serialization for types that implement standard string
/// conversion methods.
#[derive(Clone, Copy)]
pub struct Str<T>(pub T);

impl<T> Deref for Str<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> Debug for Str<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl<T> Serialize for Str<T>
where
    T: Display,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: Display,
        S: Serializer,
    {
        serializer.serialize_str(&format!("{}", self.0))
    }
}

impl<'de, T> Deserialize<'de> for Str<T>
where
    T: FromStr,
    T::Err: Display,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = Cow::<str>::deserialize(deserializer)?;
        T::from_str(&*s).map(Str).map_err(de::Error::custom)
    }
}

/// Wrapper type implementing serialization for 25b-bit unsigned intengers.
#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub struct Quantity(pub U256);

impl Debug for Quantity {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl Serialize for Quantity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{:#x}", self.0))
    }
}

impl<'de> Deserialize<'de> for Quantity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = Cow::<str>::deserialize(deserializer)?;
        let s = s
            .strip_prefix("0x")
            .ok_or_else(|| de::Error::custom("missing '0x' prefix"))?;
        U256::from_str_radix(s, 16)
            .map(Quantity)
            .map_err(de::Error::custom)
    }
}
