//! Partial transaction type for RPC calls.

use crate::serialization::{Bytes, Quantity, Str};
use anyhow::{bail, Result};
use hdwallet::account::Address;
use rocket::serde::{de, json::serde_json, Deserialize, Deserializer, Serialize, Serializer};
use std::{
    fmt::{self, Debug, Formatter},
    ops::Deref,
};

/// Transaction request parameters uses for `eth_sendTransaction` and
/// `eth_signTransaction` RPC calls.
///
/// This is basically an Ethereum transaction with a `from` field used to
/// determine the account to sign with and with optional arguments.
pub struct TransactionRequest(TransactionArgs);

/// Valid transaction kinds.
#[derive(Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
enum TransactionKind {
    #[serde(rename = "0x1")]
    Eip2930,
    #[serde(rename = "0x2")]
    Eip1559,
}

/// Inner transaction arguments with generated serialization implementation.
#[derive(Deserialize, Serialize)]
#[serde(crate = "rocket::serde", deny_unknown_fields)]
struct TransactionArgs {
    /// The transaction type.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<TransactionKind>,
    /// The account used for sending the transaction.
    #[serde(skip_serializing)]
    from: Str<Address>,
    /// The target address for the transaction. This can also be `None` to
    /// indicate a contract creation transaction.
    to: Option<Str<Address>>,
    /// The gas limit for the transaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    gas: Option<Quantity>,
    /// The gas price in Wei for the transaction.
    #[serde(rename = "gasPrice", skip_serializing_if = "Option::is_none")]
    gas_price: Option<Quantity>,
    /// The maximum gas price in Wei for the transaction.
    #[serde(rename = "maxFeePerGas", skip_serializing_if = "Option::is_none")]
    max_fee_per_gas: Option<Quantity>,
    /// The maximum priority fee in Wei for the transaction.
    #[serde(
        rename = "maxPriorityFeePerGas",
        skip_serializing_if = "Option::is_none"
    )]
    max_priority_fee_per_gas: Option<Quantity>,
    /// The amount of Ether to send with the transaction.
    #[serde(default)]
    value: Quantity,
    /// The calldata to use for the transaction.
    #[serde(default)]
    data: Bytes<Vec<u8>>,
    /// The nonce for the transaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    nonce: Option<Quantity>,
    /// List of addresses and storage keys that the transaction plans to access.
    #[serde(rename = "accessList", skip_serializing_if = "Option::is_none")]
    access_list: Option<AccessList>,
    /// The chain ID for the transaction.
    #[serde(rename = "chainId", skip_serializing_if = "Option::is_none")]
    chain_id: Option<Quantity>,
}

/// List of addresses and storage keys that the transaction plans to access.
type AccessList = Vec<(Str<Address>, Vec<Bytes<[u8; 32]>>)>;

impl TransactionRequest {
    /// Creates a transaction request from arguments.
    ///
    /// Validates that the arguments make sense and fills in some defaults.
    fn from_args(mut args: TransactionArgs) -> Result<Self> {
        match &args {
            // Prefer EIP-1559 transactions
            TransactionArgs {
                kind: None | Some(TransactionKind::Eip1559),
                gas_price: None,
                ..
            } => {
                args.kind = Some(TransactionKind::Eip1559);
                args.access_list.get_or_insert(AccessList::default());
            }
            TransactionArgs {
                kind: None,
                max_fee_per_gas: None,
                max_priority_fee_per_gas: None,
                access_list: None,
                ..
            } => {}
            TransactionArgs {
                kind: None | Some(TransactionKind::Eip2930),
                max_fee_per_gas: None,
                max_priority_fee_per_gas: None,
                ..
            } => {
                args.kind = Some(TransactionKind::Eip2930);
                args.access_list.get_or_insert(Default::default());
            }

            _ => bail!("malformed transaction args"),
        }

        Ok(Self(args))
    }

    /// Fills a transaction by computing all unspecified fields.
    pub async fn fill(self) -> Result<(Address, Transaction)> {
        todo!()
    }
}

impl<'de> Deserialize<'de> for TransactionRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let args = TransactionArgs::deserialize(deserializer)?;
        TransactionRequest::from_args(args).map_err(de::Error::custom)
    }
}

/// Inner actual `Transaction` implementation.
type Inner = hdwallet::transaction::Transaction;

/// A wrapper type around the raw transaction arguments implementing
/// serialization and debug printing.
pub struct Transaction {
    args: TransactionArgs,
    inner: Inner,
}

impl Deref for Transaction {
    type Target = Inner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Debug for Transaction {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match serde_json::to_string(self) {
            Ok(json) => f.write_str(&json),
            Err(err) => {
                tracing::error!(?err, "unexpected error formatting transaction");
                f.write_str("Transaction { FORMATTING_ERROR }")
            }
        }
    }
}

impl Serialize for Transaction {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.args.serialize(serializer)
    }
}
