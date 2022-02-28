# HD Node

Wraps an Ethereum node RPC endpoint with account managment.

This allows, for example, to run a local node RPC endpoint that proxies all
requests to say, Infura, while internally handing account-specific requests like
`eth_sendTransaction`.

## Validation

The service provides some very basic validation on the signed data:
1. The chain ID specified in the transaction and typed data must match the
   node's chain ID
2. The nonce specified in the transaction is **only the next nonce**. This means
   you can only sign one transaction at a time, but is important for foward
   secrecy.

Additionally, the service has a concept of a "validator" - a Lua module that
gets called on every signature operation to validate whether or not the
signature should be allowed. It can perform arbitrary logic. For an example,
take a look at [the CowSwap sample validator](validators/cowswap.lua).

## TODO

- [ ] CI
- [ ] Increase test coverage - its pretty poor ATM
- [ ] Optionally record signatures to a database instead of just logging them
- [ ] General project cleanup - it was done quite quickly
