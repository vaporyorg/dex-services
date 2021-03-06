pub mod stablex_auction_element;
pub mod stablex_contract;

use crate::http::HttpFactory;
use crate::transport::HttpTransport;
use anyhow::Result;
use ethcontract::contract::MethodDefaults;
use ethcontract::{Account, PrivateKey};
use std::time::Duration;

pub type Web3 = ethcontract::web3::api::Web3<HttpTransport>;

pub fn web3_provider(http_factory: &HttpFactory, url: &str, timeout: Duration) -> Result<Web3> {
    let http = HttpTransport::new(http_factory, url, timeout)?;
    let web3 = Web3::new(http);

    Ok(web3)
}

fn method_defaults(key: PrivateKey, network_id: u64) -> Result<MethodDefaults> {
    let account = Account::Offline(key, Some(network_id));
    let defaults = MethodDefaults {
        from: Some(account),
        gas: None,
        gas_price: None,
    };

    Ok(defaults)
}
