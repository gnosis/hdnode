# HD Node

Wraps an Ethereum node RPC endpoint with account managment.

This allows, for example, to run a local node RPC endpoint that proxies all
requests to say, Infura, while internally handing account-specific requests like
`eth_sendTransaction`.
