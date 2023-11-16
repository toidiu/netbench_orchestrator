// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

mod error;
mod russula;

use error::OrchResult;
use russula::netbench::client;
use russula::netbench::server;
use russula::RussulaBuilder;
use std::str::FromStr;
use std::{collections::BTreeSet, net::SocketAddr};
use structopt::{clap::arg_enum, StructOpt};

#[derive(StructOpt)]
struct Opt {
    /// specify the protocol
    #[structopt(possible_values = &RussulaProtocol::variants(), case_insensitive = true, long, short)]
    protocol: RussulaProtocol,
}

arg_enum! {
enum RussulaProtocol {
    NetbenchClientWorker,
    NetbenchServerWorker,
}
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> OrchResult<()> {
    let opt = Opt::from_args();

    tracing_subscriber::fmt::init();

    match opt.protocol {
        RussulaProtocol::NetbenchClientWorker => run_client_worker().await,
        RussulaProtocol::NetbenchServerWorker => run_server_worker().await,
    };

    println!("hi");
    Ok(())
}

async fn run_client_worker() {
    let w1_sock = SocketAddr::from_str("127.0.0.1:8991").unwrap();
    let protocol = client::WorkerProtocol::new(w1_sock.port());
    let worker = RussulaBuilder::new(BTreeSet::from_iter([w1_sock]), protocol);
    let _worker = worker.build().await.unwrap();
}

async fn run_server_worker() {
    let w1_sock = SocketAddr::from_str("127.0.0.1:8991").unwrap();
    let protocol = server::WorkerProtocol::new(w1_sock.port());
    let worker = RussulaBuilder::new(BTreeSet::from_iter([w1_sock]), protocol);
    let _coord = worker.build().await.unwrap();
}
