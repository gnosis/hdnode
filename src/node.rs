//! Module implemeting the HD node handler.

use crate::serialization;
use hdwallet::account::{Address, PrivateKey};
use hyper::{Client, Uri};
use serde::{Deserialize, Serialize};

/// HD Node.
pub struct Node {
    accounts: Vec<PrivateKey>,
    //fallback: (Client, Uri),
}

/// Supported JSON RPC methods and parameters.
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "method", content = "params")]
pub enum Method {
    #[serde(rename = "eth_sign")]
    Sign(
        #[serde(with = "serialization::address")] Address,
        #[serde(with = "serialization::bytes")] Vec<u8>,
    ),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn calls() {
        assert_eq!(
            serde_json::to_value(&Method::Sign(Address([0x42; 20]), vec![1, 3, 3, 7])).unwrap(),
            json!({
                "method": "eth_sign",
                "params": ["0x4242424242424242424242424242424242424242", "0x01030307"],
            }),
        );
    }
}
