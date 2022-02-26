//! A signer that just logs all signing operations.

use super::{Signing, Transaction, TypedData};
use anyhow::Result;
use hdwallet::account::{Address, Signature};

/// Wrapping signer that logs all signing operations to the global logger.
pub struct LogRecorder<S>(pub S);

impl<S> Signing for LogRecorder<S>
where
    S: Signing,
{
    fn accounts(&self) -> &[Address] {
        self.0.accounts()
    }

    fn sign_message(&self, account: Address, message: &[u8]) -> Result<Signature> {
        let signature = self.0.sign_message(account, message)?;
        let message = format!("0x{}", hex::encode(message));
        tracing::info!(%account, %message, %signature, "signed message");
        Ok(signature)
    }

    fn sign_transaction(&self, account: Address, transaction: &Transaction) -> Result<Signature> {
        let signature = self.0.sign_transaction(account, transaction)?;
        tracing::info!(%account, ?transaction, %signature, "signed transaction");
        Ok(signature)
    }

    fn sign_typed_data(&self, account: Address, typed_data: &TypedData) -> Result<Signature> {
        let signature = self.0.sign_typed_data(account, typed_data)?;
        tracing::info!(%account, ?typed_data, %signature, "signed typed data");
        Ok(signature)
    }
}
