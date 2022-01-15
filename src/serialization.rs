//! Module containing serialization helpers.

/// Dynamic byte array serialization methods.
pub mod bytes {
    use serde::{
        de::{self, Deserializer},
        ser::Serializer,
        Deserialize as _,
    };
    use std::borrow::Cow;

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("0x{}", hex::encode(bytes)))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = Cow::<str>::deserialize(deserializer)?;
        let s = s
            .strip_prefix("0x")
            .ok_or_else(|| de::Error::custom("storage slot missing '0x' prefix"))?;
        hex::decode(s).map_err(de::Error::custom)
    }
}

/// Address serialization.
///
/// `hdwallet` crate has partial support for serialization, this fills in the
/// gaps.
pub mod address {
    use hdwallet::account::Address;
    use serde::{de::Deserializer, ser::Serializer, Deserialize as _};

    pub fn serialize<S>(address: &Address, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&address.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Address, D::Error>
    where
        D: Deserializer<'de>,
    {
        Address::deserialize(deserializer)
    }
}

/// Result serialization.
pub mod result {
    use serde::{
        de::{self, Deserializer},
        ser::Serializer,
        Deserialize, Serialize,
    };

    #[derive(Deserialize, Serialize)]
    struct Res<T, E> {
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<T>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<E>,
    }

    pub fn serialize<S, T, E>(result: &Result<T, E>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: Serialize,
        E: Serialize,
    {
        match result {
            Ok(result) => Res {
                result: Some(result),
                error: None,
            },
            Err(error) => Res {
                result: None,
                error: Some(error),
            },
        }
        .serialize(serializer)
    }

    pub fn deserialize<'de, D, T, E>(deserializer: D) -> Result<Result<T, E>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
        E: Deserialize<'de>,
    {
        match Res::<T, E>::deserialize(deserializer)? {
            Res {
                result: Some(result),
                error: None,
            } => Ok(Ok(result)),
            Res {
                result: None,
                error: Some(error),
            } => Ok(Err(error)),
            Res {
                result: None,
                error: None,
            } => Err(de::Error::custom("missing result or error")),
            Res {
                result: Some(_),
                error: Some(_),
            } => Err(de::Error::custom("both result and error specified")),
        }
    }
}
