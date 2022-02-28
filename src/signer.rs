//! An abstraction around signers.
//!
//! This allows us to compose different operations around various signing
//! methods, such as validating transaction signatures and recording them to a
//! database.

pub mod log_recorder;
pub mod validator;
pub mod wallet;

use crate::node::{transaction::Transaction, typeddata::TypedData};
use anyhow::Result;
use hdwallet::account::{Address, Signature};

/// A trait abstracting Ethereum signing methods.
pub trait Signing {
    /// Returns the list of addresses of the accounts managed by the signer.
    fn accounts(&self) -> &[Address];

    /// Signs an Ethereum message.
    fn sign_message(&self, account: Address, message: &[u8]) -> Result<Signature>;

    /// Signs an Ethereum transaction.
    fn sign_transaction(&self, account: Address, transaction: &Transaction) -> Result<Signature>;

    /// Signs Ethereum EIP-712 typed data.
    fn sign_typed_data(&self, account: Address, typed_data: &TypedData) -> Result<Signature>;
}

/// A boxed signer that is safe to send between threads.
pub type BoxSigner = Box<dyn Signing + Send + Sync + 'static>;
