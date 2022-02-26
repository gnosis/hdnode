//! Module implementing serializers for Ethereum JSON RPC methods.

use crate::{
    jsonrpc::{Id, JsonRpc, Params, Request, Response},
    serialization::{NoParameters, Quantity},
};
use anyhow::{bail, Result};
use ethnum::U256;
use rocket::serde::{
    json::{self, serde_json, Value},
    Serialize,
};
use std::{
    future::Future,
    sync::atomic::{AtomicU64, Ordering},
};

static ID: AtomicU64 = AtomicU64::new(0);

fn prepare(method: impl Into<String>, params: impl Serialize) -> Result<Request> {
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

pub async fn chain_id<F, Fut>(f: F) -> Result<U256>
where
    F: FnOnce(Request) -> Fut,
    Fut: Future<Output = Result<Response>>,
{
    let request = prepare("eth_chainId", NoParameters::default())?;
    let response = f(request).await?;
    let result = json::from_value::<Quantity>(response.result?)?;
    Ok(result.0)
}
