//! The wallet used for performing HD node operations.

use anyhow::{Context as _, Result};
use hdwallet::{
    account::{Address, PrivateKey, Signature},
    hdk,
    message::EthereumMessage,
    mnemonic::Mnemonic,
    transaction::Transaction,
    typeddata::TypedData,
};
use std::collections::HashMap;
use thiserror::Error;

/// A collection of accounts that can perform Ethereum ECDSA operations.
pub struct Wallet {
    addresses: Vec<Address>,
    accounts: HashMap<[u8; 20], PrivateKey>,
}

impl Wallet {
    /// Creates a new wallet from a mnemonic, generating private keys for the
    /// specified number of accounts.
    pub fn new(mnemonic: &Mnemonic, password: &str, count: usize) -> Result<Self> {
        let seed = mnemonic.seed(password);
        let private_keys = (0..count)
            .map(|i| hdk::derive_index(&seed, i))
            .collect::<Result<Vec<_>, _>>()
            .context("key derivation error")?;

        let addresses = private_keys.iter().map(|key| key.address()).collect();
        let accounts = private_keys
            .into_iter()
            .map(|account| (account.address().0, account))
            .collect();

        Ok(Self {
            addresses,
            accounts,
        })
    }

    /// Returns the list of addresses kept in the node wallet.
    pub fn accounts(&self) -> &[Address] {
        &self.addresses
    }

    /// Signs an Ethereum message.
    pub fn sign_message(
        &self,
        account: Address,
        message: &[u8],
    ) -> Result<Signature, UnknownSignerError> {
        let message = EthereumMessage(message);
        self.sign(account, message.signing_message())
    }

    /// Signs an Ethereum message.
    pub fn sign_transaction(
        &self,
        account: Address,
        transaction: &Transaction,
    ) -> Result<Signature, UnknownSignerError> {
        self.sign(account, transaction.signing_message())
    }

    /// Signs an Ethereum message.
    pub fn sign_typed_data(
        &self,
        account: Address,
        typed_data: &TypedData,
    ) -> Result<Signature, UnknownSignerError> {
        self.sign(account, typed_data.signing_message())
    }

    /// Signs a raw message with the specified account.
    fn sign(
        &self,
        account: Address,
        signing_message: [u8; 32],
    ) -> Result<Signature, UnknownSignerError> {
        let private_key = self
            .accounts
            .get(&account.0)
            .ok_or(UnknownSignerError(account))?;
        Ok(private_key.sign(signing_message))
    }
}

/// An error indicating that the signer is unknown.
#[derive(Debug, Error)]
#[error("unknown signer {0}")]
pub struct UnknownSignerError(pub Address);
