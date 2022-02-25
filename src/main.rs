mod jsonrpc;
mod node;
mod serialization;
mod wallet;

use self::{node::Node, wallet::Wallet};
use clap::Parser;
use hdwallet::mnemonic::{Language, Mnemonic};
use reqwest::Url;

const VERSION: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

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

    /// The remote node being proxied.
    #[clap(long, env)]
    remote_node_url: Url,
}

#[rocket::main]
async fn main() {
    let args = Args::parse();
    tracing_subscriber::fmt::init();

    let mnemonic = args.mnemonic.unwrap_or_else(|| {
        let mnemonic = Mnemonic::random(Language::English, 12).unwrap();
        tracing::info!(%mnemonic, "using random mnemonic");
        mnemonic
    });
    let wallet = Wallet::new(&mnemonic, &args.password, args.account_count).unwrap();
    tracing::debug!(accounts = ?wallet.accounts(), "derived accounts");

    let remote = jsonrpc::Client::new(args.remote_node_url).unwrap();
    tracing::debug!(url = ?remote.url(), "connected to remote node");

    let figment = rocket::Config::figment().merge(("port", 8545));
    rocket::custom(figment)
        .manage(Node::new(wallet, remote))
        .mount(
            "/",
            rocket::routes![node::request, node::batch, node::error],
        )
        .launch()
        .await
        .unwrap();
}
