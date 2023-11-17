// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

mod error;
mod russula;

use core::time::Duration;
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
    #[structopt(long, default_value = "8888")]
    port: u16,
}

arg_enum! {
#[allow(clippy::enum_variant_names)]
enum RussulaProtocol {
    NetbenchServerWorker,
    NetbenchServerCoordinator,
    NetbenchClientWorker,
    NetbenchClientCoordinator,
}
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> OrchResult<()> {
    let opt = Opt::from_args();

    let file_appender = tracing_appender::rolling::hourly("./target", "russula.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
   tracing_subscriber::fmt()
       .with_writer(non_blocking)
       .init();

    match opt.protocol {
        RussulaProtocol::NetbenchServerWorker => run_server_worker(opt.port).await,
        RussulaProtocol::NetbenchServerCoordinator => run_server_coordinator(opt.port).await,
        RussulaProtocol::NetbenchClientWorker => run_client_worker(opt.port).await,
        RussulaProtocol::NetbenchClientCoordinator => run_client_coordinator(opt.port).await,
    };

    println!("hi");
    Ok(())
}

async fn run_server_worker(port: u16) {
    let w1_sock = SocketAddr::from_str(&format!("127.0.0.1:{}", port)).unwrap();
    let protocol = server::WorkerProtocol::new(port);
    let worker = RussulaBuilder::new(BTreeSet::from_iter([w1_sock]), protocol);
    let mut worker = worker.build().await.unwrap();
    worker.run_till_ready().await;

    worker
        .run_till_state(server::WorkerState::Done, || {
            // println!("[server-worker-1] run-------loop till state: Done---------");
        })
        .await
        .unwrap();
}

async fn run_server_coordinator(port: u16) {
    let w1_sock = SocketAddr::from_str(&format!("127.0.0.1:{}", port)).unwrap();
    let protocol = server::CoordProtocol::new();
    let coord = RussulaBuilder::new(BTreeSet::from_iter([w1_sock]), protocol);
    let mut coord = coord.build().await.unwrap();

    coord
        .run_till_state(server::CoordState::WorkersRunning, || {})
        .await
        .unwrap();

    println!("[server-coord-1] sleeping --------- to allow worker to run");
    tokio::time::sleep(Duration::from_secs(5)).await;

    coord
        .run_till_state(server::CoordState::Done, || {})
        .await
        .unwrap();
}

async fn run_client_worker(port: u16) {
    let w1_sock = SocketAddr::from_str(&format!("127.0.0.1:{}", port)).unwrap();
    let protocol = client::WorkerProtocol::new(port);
    let worker = RussulaBuilder::new(BTreeSet::from_iter([w1_sock]), protocol);
    let mut worker = worker.build().await.unwrap();
    worker.run_till_ready().await;

    worker
        .run_till_state(client::WorkerState::Done, || {
            // println!("[server-worker-1] run-------loop till state: Done---------");
        })
        .await
        .unwrap();
}

async fn run_client_coordinator(port: u16) {
    let w1_sock = SocketAddr::from_str(&format!("127.0.0.1:{}", port)).unwrap();
    let protocol = client::CoordProtocol::new();
    let coord = RussulaBuilder::new(BTreeSet::from_iter([w1_sock]), protocol);
    let mut coord = coord.build().await.unwrap();

    coord
        .run_till_state(client::CoordState::WorkersRunning, || {})
        .await
        .unwrap();

    println!("[client-coord-1] sleeping --------- to allow worker to run");
    tokio::time::sleep(Duration::from_secs(5)).await;

    coord
        .run_till_state(client::CoordState::Done, || {})
        .await
        .unwrap();
}
