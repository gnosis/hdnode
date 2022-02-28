//! Signature validation.

use crate::serialization::Bytes;

use super::{Signing, Transaction, TypedData};
use anyhow::{ensure, Context as _, Result};
use hdwallet::account::{Address, Signature};
use mlua::{Function, Lua, LuaSerdeExt as _, StdLib, Value, Variadic};
use rocket::serde::Serialize;
use std::{fs, path::Path, sync::Mutex};

/// A validating signer whose logic is defined by a Lua module.
pub struct Validator<S> {
    lua: Mutex<Lua>,
    inner: S,
}

impl<S> Validator<S> {
    /// Creates a new validator wrapping the specified signer and using the
    /// specified path as a Lua module for validation logic.
    pub fn new(inner: S, module: &Path) -> Result<Self> {
        let lua = Lua::new_with(
            StdLib::TABLE | StdLib::STRING | StdLib::UTF8 | StdLib::MATH,
            Default::default(),
        )?;

        // Override `print` function and forward it to logs.
        let print = lua.create_function(|lua, values: Variadic<Value>| {
            let mut buffer = String::new();
            for (i, value) in values.iter().enumerate() {
                if i > 0 {
                    buffer.push('\t');
                }
                if let Some(string) = lua.coerce_string(value.clone())? {
                    buffer.push_str(&string.to_string_lossy());
                }
            }
            tracing::debug!("{buffer}");
            Ok(())
        })?;
        lua.globals().set("print", print)?;

        let src = fs::read_to_string(module)?;
        lua.load(&src).set_name("validator")?.exec()?;

        Ok(Self {
            lua: Mutex::new(lua),
            inner,
        })
    }

    fn validate<T>(&self, name: &str, account: Address, data: &T) -> Result<()>
    where
        T: Serialize,
    {
        let lua = self.lua.lock().unwrap();
        let handler = lua
            .globals()
            .get::<_, Function>(name)
            .with_context(|| format!("missing '{name}' handler in module"))?;
        let input = (account.to_string(), lua.to_value(data)?);
        let ok = handler.call::<_, bool>(input)?;
        ensure!(ok, "handler '{name}' denied signature");

        Ok(())
    }

    fn validate_message(&self, account: Address, message: &[u8]) -> Result<()> {
        self.validate("validate_message", account, &Bytes(message))
    }

    fn validate_transaction(&self, account: Address, transaction: &Transaction) -> Result<()> {
        self.validate("validate_transaction", account, transaction)
    }

    fn validate_typed_data(&self, account: Address, typed_data: &TypedData) -> Result<()> {
        self.validate("validate_typed_data", account, typed_data)
    }
}

impl<S> Signing for Validator<S>
where
    S: Signing,
{
    fn accounts(&self) -> &[Address] {
        self.inner.accounts()
    }

    fn sign_message(&self, account: Address, message: &[u8]) -> Result<Signature> {
        self.validate_message(account, message)?;
        self.inner.sign_message(account, message)
    }

    fn sign_transaction(&self, account: Address, transaction: &Transaction) -> Result<Signature> {
        self.validate_transaction(account, transaction)?;
        self.inner.sign_transaction(account, transaction)
    }

    fn sign_typed_data(&self, account: Address, typed_data: &TypedData) -> Result<Signature> {
        self.validate_typed_data(account, typed_data)?;
        self.inner.sign_typed_data(account, typed_data)
    }
}
