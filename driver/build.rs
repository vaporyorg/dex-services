use ethcontract_generate::Builder;
use std::env;
use std::path::Path;

fn main() {
    generate_contract("BatchExchange", "batch_exchange.rs");
    generate_contract("BatchExchangeViewer", "batch_exchange_viewer.rs");
}

fn generate_contract(name: &str, out: &str) {
    let artifact = format!("../dex-contracts/build/contracts/{}.json", name);
    let dest = env::var("OUT_DIR").unwrap();
    println!("cargo:rerun-if-changed={}", artifact);
    Builder::new(artifact)
        .with_visibility_modifier(Some("pub"))
        .add_event_derive("serde::Deserialize")
        .add_event_derive("serde::Serialize")
        .generate()
        .unwrap()
        .write_to_file(Path::new(&dest).join(out))
        .unwrap();
}
