//! Partial transaction type for RPC calls.

use crate::{
    node::{eth::Eth, types::Block},
    serialization::{Bytes, Quantity, Str},
};
use anyhow::{ensure, Result};
use hdwallet::{
    account::Address,
    transaction::{Eip1559Transaction, Eip2930Transaction, LegacyTransaction},
};
use rocket::serde::{json::serde_json, Deserialize, Serialize, Serializer};
use std::{
    fmt::{self, Debug, Formatter},
    ops::Deref,
};

/// Transaction request parameters uses for `eth_sendTransaction` and
/// `eth_signTransaction` RPC calls.
///
/// This is basically an Ethereum transaction with a `from` field used to
/// determine the account to sign with and with optional arguments.
#[derive(Clone, Deserialize, Serialize)]
#[serde(crate = "rocket::serde", deny_unknown_fields)]
pub struct TransactionRequest {
    /// The account used for sending the transaction.
    #[serde(skip_serializing)]
    pub from: Str<Address>,
    /// The target address for the transaction. This can also be `None` to
    /// indicate a contract creation transaction.
    pub to: Option<Str<Address>>,
    /// The gas limit for the transaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas: Option<Quantity>,
    /// The gas price in Wei for the transaction.
    #[serde(rename = "gasPrice", skip_serializing_if = "Option::is_none")]
    pub gas_price: Option<Quantity>,
    /// The maximum gas price in Wei for the transaction.
    #[serde(rename = "maxFeePerGas", skip_serializing_if = "Option::is_none")]
    pub max_fee_per_gas: Option<Quantity>,
    /// The maximum priority fee in Wei for the transaction.
    #[serde(
        rename = "maxPriorityFeePerGas",
        skip_serializing_if = "Option::is_none"
    )]
    pub max_priority_fee_per_gas: Option<Quantity>,
    /// The amount of Ether to send with the transaction.
    #[serde(default)]
    pub value: Quantity,
    /// The calldata to use for the transaction.
    #[serde(default)]
    pub data: Bytes<Vec<u8>>,
    /// The nonce for the transaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<Quantity>,
    /// List of addresses and storage keys that the transaction plans to access.
    #[serde(rename = "accessList", skip_serializing_if = "Option::is_none")]
    pub access_list: Option<AccessList>,
    /// The chain ID for the transaction.
    #[serde(rename = "chainId", skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<Quantity>,
}

/// List of addresses and storage keys that the transaction plans to access.
type AccessList = Vec<(Str<Address>, Vec<Bytes<[u8; 32]>>)>;

impl TransactionRequest {
    /// Fills a transaction by computing all unspecified fields.
    pub async fn fill(mut self, eth: &Eth) -> Result<(Address, Transaction)> {
        let account = self.from.0;

        let mut batch = eth.batch();
        let chain_id = batch.chain_id();
        let nonce = batch.get_transaction_count(account, Block::Latest);

        let gas = match self.gas {
            None => Some(batch.estimate_gas(self.clone(), Block::Pending)),
            _ => None,
        };

        let gas_parameters = (
            self.gas_price,
            self.max_fee_per_gas,
            self.max_priority_fee_per_gas,
        );
        ensure!(
            matches!(gas_parameters, (None, _, _) | (_, None, None)),
            "specified both gas price and London gas parameters",
        );
        let gas_price = match gas_parameters {
            (None, None, None) => Some(batch.gas_price()),
            _ => None,
        };
        let base_fee = match gas_parameters {
            (None, None, _) => Some(batch.base_fee()),
            _ => None,
        };
        let max_priority_fee_per_gas = match gas_parameters {
            (None, _, None) => Some(batch.max_priority_fee_per_gas()),
            _ => None,
        };

        batch.execute().await?;

        let chain_id = chain_id.await?;
        ensure!(
            self.chain_id.get_or_insert(Quantity(chain_id)).0 == chain_id,
            "chain ID used for signing does not match node"
        );
        let nonce = nonce.await?;
        ensure!(
            self.nonce.get_or_insert(Quantity(nonce)).0 == nonce,
            "only signing transactions for current nonce ({nonce:#x}) permitted",
        );

        if let Some(gas) = gas {
            self.gas = Some(Quantity(gas.await?));
        }
        match (gas_price, base_fee, max_priority_fee_per_gas) {
            (Some(gas_price), Some(base_fee), Some(max_priority_fee_per_gas)) => {
                // Prefer EIP-1559 gas pricing, but fallback to legacy gas
                // pricing if not supported by nodes.
                match (base_fee.await, max_priority_fee_per_gas.await) {
                    (Ok(base_fee), Ok(max_priority_fee_per_gas)) => {
                        self.max_fee_per_gas =
                            Some(Quantity(base_fee * 2 + max_priority_fee_per_gas));
                        self.max_priority_fee_per_gas = Some(Quantity(max_priority_fee_per_gas));
                    }
                    _ => {
                        self.gas_price = Some(Quantity(gas_price.await?));
                    }
                }
            }
            (gas_price, base_fee, max_priority_fee_per_gas) => {
                if let Some(gas_price) = gas_price {
                    self.gas_price = Some(Quantity(gas_price.await?));
                }
                if let Some(max_priority_fee_per_gas) = max_priority_fee_per_gas {
                    self.max_priority_fee_per_gas = Some(Quantity(max_priority_fee_per_gas.await?));
                }
                if let Some(base_fee) = base_fee {
                    self.max_fee_per_gas = Some(Quantity(
                        base_fee.await? * 2 + self.max_priority_fee_per_gas.unwrap().0,
                    ));
                }
            }
        }

        Ok((account, Transaction::from_args(self)))
    }

    /// Returns the access list for this transaction request in the `hdwallet`
    /// format.
    fn hdwallet_access_list(&self) -> hdwallet::transaction::accesslist::AccessList {
        hdwallet::transaction::accesslist::AccessList(
            self.access_list
                .iter()
                .flatten()
                .map(|(Str(address), slots)| {
                    (
                        *address,
                        slots
                            .iter()
                            .map(|Bytes(slot)| {
                                hdwallet::transaction::accesslist::StorageSlot(*slot)
                            })
                            .collect(),
                    )
                })
                .collect(),
        )
    }
}

/// Inner actual `Transaction` implementation.
type Inner = hdwallet::transaction::Transaction;

/// A wrapper type around the raw transaction arguments implementing
/// serialization and debug printing.
pub struct Transaction {
    args: TransactionRequest,
    inner: Inner,
}

impl Transaction {
    /// Creates a new instance from a **filled** transaction request.
    ///
    /// # Panics
    ///
    /// Panics if fields are missing.
    fn from_args(args: TransactionRequest) -> Self {
        let inner = match (&args.max_fee_per_gas, &args.access_list) {
            (Some(_), _) => Inner::Eip1559(Eip1559Transaction {
                chain_id: args.chain_id.unwrap().0,
                nonce: args.nonce.unwrap().0,
                max_priority_fee_per_gas: args.max_priority_fee_per_gas.unwrap().0,
                max_fee_per_gas: args.max_fee_per_gas.unwrap().0,
                gas_limit: args.gas.unwrap().0,
                to: args.to.map(|to| to.0),
                value: args.value.0,
                data: args.data.0.clone(),
                access_list: args.hdwallet_access_list(),
            }),
            (None, Some(_)) => Inner::Eip2930(Eip2930Transaction {
                chain_id: args.chain_id.unwrap().0,
                nonce: args.nonce.unwrap().0,
                gas_price: args.gas_price.unwrap().0,
                gas_limit: args.gas.unwrap().0,
                to: args.to.map(|to| to.0),
                value: args.value.0,
                data: args.data.0.clone(),
                access_list: args.hdwallet_access_list(),
            }),
            (None, None) => Inner::Legacy(LegacyTransaction {
                nonce: args.nonce.unwrap().0,
                gas_price: args.gas_price.unwrap().0,
                gas_limit: args.gas.unwrap().0,
                to: args.to.map(|to| to.0),
                value: args.value.0,
                data: args.data.0.clone(),
                chain_id: Some(args.chain_id.unwrap().0),
            }),
        };

        Self { args, inner }
    }
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
