//! Module implementing serializers for Ethereum JSON RPC methods.

use crate::{
    jsonrpc::{self, Id, JsonRpc, Params, Request, Response},
    node::{
        transaction::TransactionRequest,
        types::{Block, FeeHistory},
    },
    serialization::{NoParameters, Quantity, Str},
};
use anyhow::{bail, Result};
use ethnum::U256;
use hdwallet::account::Address;
use reqwest::Url;
use rocket::{
    serde::{
        json::{self, serde_json, Value},
        DeserializeOwned, Serialize,
    },
    tokio::sync::oneshot,
};
use std::{
    future::Future,
    ops::Deref,
    sync::atomic::{AtomicU64, Ordering},
};

static ID: AtomicU64 = AtomicU64::new(1);

/// Prepares a request.
fn prepare(method: &'static str, params: impl Serialize) -> Result<Request> {
    let params = match serde_json::to_value(params)? {
        Value::Array(array) => Params::Array(array),
        other => bail!("invalid Ethereum JSON RPC parameters {other}"),
    };
    let id = ID.fetch_add(1, Ordering::SeqCst);

    Ok(Request {
        jsonrpc: JsonRpc::V2,
        method: method.into(),
        params: Some(params),
        id: Id::Number(id.into()),
    })
}

/// An Ethereum RPC client.
pub struct Eth {
    client: jsonrpc::Client,
}

impl Eth {
    /// Creates a new Ethereum RPC client.
    pub fn new(client: jsonrpc::Client) -> Self {
        Self { client }
    }

    /// Creates a new Ethereum RPC client from a URL.
    pub fn from_url(url: Url) -> Result<Self> {
        Ok(Self::new(jsonrpc::Client::new(url)?))
    }

    /// Creates a new batch of Ethereum RPC calls.
    pub fn batch(&self) -> Batch<'_> {
        Batch {
            client: &self.client,
            queue: Vec::new(),
        }
    }

    /// Performs an RPC call immediately.
    async fn call<I, O>(&self, method: &'static str, params: I) -> Result<O>
    where
        I: Serialize,
        O: DeserializeOwned,
    {
        let request = prepare(method, params)?;
        let response = self.client.execute(&request).await?;
        let result = json::from_value::<O>(response.result?)?;
        Ok(result)
    }

    /// Retrieves the chain ID.
    pub async fn chain_id(&self) -> Result<U256> {
        Ok(self
            .call::<_, Quantity>("eth_chainId", NoParameters::default())
            .await?
            .0)
    }
}

impl Deref for Eth {
    type Target = jsonrpc::Client;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

/// A batched Ethereum RPC client.
pub struct Batch<'a> {
    client: &'a jsonrpc::Client,
    queue: Vec<(Request, oneshot::Sender<Response>)>,
}

impl<'a> Batch<'a> {
    /// Executes the batch, causing all call futures to progress.
    pub async fn execute(self) -> Result<()> {
        let (requests, channels): (Vec<_>, Vec<_>) = self.queue.into_iter().unzip();
        let responses = self.client.execute_many(&requests).await?;
        for (channel, response) in channels.into_iter().zip(responses) {
            let _ = channel.send(response);
        }

        Ok(())
    }

    /// Adds a call to the batch and returns a future that resolves once it gets
    /// executed.
    fn call<I, O>(&mut self, method: &'static str, params: I) -> impl Future<Output = Result<O>>
    where
        I: Serialize,
        O: DeserializeOwned,
    {
        let request = prepare(method, params);
        let response = request.map(|request| {
            let (response_tx, response_rx) = oneshot::channel();
            self.queue.push((request, response_tx));
            response_rx
        });

        async move {
            let response = response?.await?;
            let result = json::from_value::<O>(response.result?)?;
            Ok(result)
        }
    }

    /// Retrieves the chain ID.
    pub fn chain_id(&mut self) -> impl Future<Output = Result<U256>> {
        let response = self.call::<_, Quantity>("eth_chainId", NoParameters::default());
        async move { Ok(response.await?.0) }
    }

    /// Retrieves an accounts transaction count (i.e. their next nonce).
    pub fn get_transaction_count(
        &mut self,
        account: Address,
        block: Block,
    ) -> impl Future<Output = Result<U256>> {
        let response = self.call::<_, Quantity>("eth_getTransactionCount", (Str(account), block));
        async move { Ok(response.await?.0) }
    }

    /// Retrieves an accounts transaction count (i.e. their next nonce).
    pub fn estimate_gas(
        &mut self,
        transaction: TransactionRequest,
        block: Block,
    ) -> impl Future<Output = Result<U256>> {
        let response = self.call::<_, Quantity>("eth_estimateGas", (transaction, block));
        async move { Ok(response.await?.0) }
    }

    /// Estimates a reasonable max priority fee to use for transactions.
    pub fn max_priority_fee_per_gas(&mut self) -> impl Future<Output = Result<U256>> {
        let response =
            self.call::<_, Quantity>("eth_maxPriorityFeePerGas", NoParameters::default());
        async move { Ok(response.await?.0) }
    }

    /// Returns the base fee for the next block.
    pub fn base_fee(&mut self) -> impl Future<Output = Result<U256>> {
        let response = self.call::<_, FeeHistory>(
            "eth_feeHistory",
            (Quantity(U256::new(1)), Block::Latest, <[f64; 0]>::default()),
        );
        async move { Ok(response.await?.base_fee_per_gas[1].0) }
    }

    /// Estimates a legacy gas price to use for transactions.
    pub fn gas_price(&mut self) -> impl Future<Output = Result<U256>> {
        let response = self.call::<_, Quantity>("eth_gasPrice", NoParameters::default());
        async move { Ok(response.await?.0) }
    }
}
