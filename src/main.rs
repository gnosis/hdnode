mod jsonrpc;
mod node;
mod serialization;
mod signer;

use crate::{
    node::Node,
    serialization::{Addresses, Str},
    signer::{log_recorder::LogRecorder, wallet::Wallet, Signing as _},
};
use anyhow::Result;
use hdwallet::mnemonic::Mnemonic;
use reqwest::Url;
use rocket::{fairing::AdHoc, serde::Deserialize};

const VERSION: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Deserialize)]
#[serde(crate = "rocket::serde")]
struct Config {
    /// The BIP-0039 mnemonic phrase for seeding the HD wallet accounts.
    mnemonic: Str<Mnemonic>,

    /// The password to use with the mnemonic phrase for salting the seed used
    /// for the HD wallet.
    #[serde(default)]
    password: String,

    /// The number of accounts to derive from the mnemonic seed phrase.
    account_count: usize,

    /// The remote node being proxied.
    remote_node_url: Str<Url>,
}

#[rocket::main]
async fn main() {
    tracing_subscriber::fmt::init();

    rocket::build()
        .attach(AdHoc::config::<Config>())
        .attach(AdHoc::try_on_ignite("hdnode::Node", |rocket| async {
            match init(rocket.state().unwrap()) {
                Ok(node) => Ok(rocket.manage(node)),
                Err(err) => {
                    tracing::error!(?err, "failed to inialize node");
                    Err(rocket)
                }
            }
        }))
        .mount("/", rocket::routes![node::handler])
        .launch()
        .await
        .unwrap();
}

fn init(config: &Config) -> Result<Node> {
    let wallet = Wallet::new(&*config.mnemonic, &config.password, config.account_count)?;
    let signer = Box::new(LogRecorder(wallet));
    tracing::debug!(accounts = ?Addresses(signer.accounts()), "derived accounts");

    let remote = jsonrpc::Client::new(config.remote_node_url.clone()).unwrap();
    tracing::debug!(url = %remote.url(), "connected to remote node");

    Ok(Node::new(signer, remote))
}
