//! Additional Ethereum RPC types.

use crate::serialization::Quantity;
use rocket::serde::{Deserialize, Serialize};

/// A block reference.
#[derive(Deserialize, Serialize)]
#[serde(crate = "rocket::serde", untagged)]
pub enum Block {
    /// The latest block.
    #[serde(rename = "latest")]
    Latest,
    /// The pending block.
    #[serde(rename = "pending")]
    Pending,
    /// The specified block number.
    Number(Quantity),
}

/// Fee history.
#[derive(Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct FeeHistory {
    /// Base fee per block.
    #[serde(rename = "baseFeePerGas")]
    pub base_fee_per_gas: Vec<Quantity>,
    /// Ratio of gas used to the block limit.
    #[serde(rename = "gasUsedRatio")]
    pub gas_used_ratio: Vec<Quantity>,
    /// The number of the oldest block included in the fee history.
    #[serde(rename = "oldestBlock")]
    pub oldest_block: Quantity,
    /// Effective priority fee reward percentiles.
    pub reward: Option<Vec<Vec<Quantity>>>,
}
