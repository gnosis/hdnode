mod node;
mod rpc;
mod serialization;

use clap::Parser;
use hdwallet::mnemonic::Mnemonic;
use hyper::Uri;
use std::net::SocketAddr;

#[derive(Debug, Parser)]
struct Args {
    /// The BIP-0039 mnemonic phrase for seeding the HD wallet accounts.
    #[clap(short, long, env, hide_env_values = true)]
    mnemonic: Option<Mnemonic>,

    /// The password to use with the mnemonic phrase for salting the seed used
    /// for the HD wallet.
    #[clap(long, env, hide_env_values = true, default_value_t)]
    password: String,

    /// The number of accounts to derive from the mnemonic seed phrase.
    #[clap(long, env, default_value_t = 100)]
    account_count: usize,

    /// The listening address for the HD node.
    #[clap(long, env, default_value_t = SocketAddr::from(([127, 0, 0, 1], 8545)))]
    listen_address: SocketAddr,

    /// The node being proxied.
    #[clap(long, env)]
    node_url: Uri,
}

fn main() {
    tracing_subscriber::fmt::init();
}
