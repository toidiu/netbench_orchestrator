// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

mod error;
mod russula;

use error::OrchResult;
use russula::{
    netbench::{client, server},
    RussulaBuilder,
};
use std::{collections::BTreeSet, net::SocketAddr, str::FromStr};
use structopt::{clap::arg_enum, StructOpt};

#[derive(StructOpt)]
struct Opt {
    /// specify the protocol
    #[structopt(possible_values = &RussulaProtocol::variants(), case_insensitive = true, long)]
    protocol: RussulaProtocol,

    /// specify the port
    #[structopt(long)]
    port: u16,
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
        RussulaProtocol::NetbenchClientWorker => run_client_worker(opt.port).await,
        RussulaProtocol::NetbenchServerWorker => run_server_worker(opt.port).await,
    };

    println!("hi");
    Ok(())
}

async fn run_client_worker(port: u16) {
    let w1_sock = SocketAddr::from_str(&format!("127.0.0.1:{}", port)).unwrap();
    let protocol = client::WorkerProtocol::new(port);
    let worker = RussulaBuilder::new(BTreeSet::from_iter([w1_sock]), protocol);
    let _worker = worker.build().await.unwrap();
}

async fn run_server_worker(port: u16) {
    let w1_sock = SocketAddr::from_str(&format!("127.0.0.1:{}", port)).unwrap();
    let protocol = server::WorkerProtocol::new(port);
    let worker = RussulaBuilder::new(BTreeSet::from_iter([w1_sock]), protocol);
    let _coord = worker.build().await.unwrap();
}
