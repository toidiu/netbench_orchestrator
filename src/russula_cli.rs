// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{duration::parse_duration, russula::netbench};
use core::time::Duration;
use error::OrchResult;
use russula::{
    netbench::{client, server},
    RussulaBuilder,
};
use std::{collections::BTreeSet, net::SocketAddr};
use structopt::StructOpt;
use tracing::debug;
use tracing_subscriber::EnvFilter;

mod duration;
mod error;
mod russula;

/// This utility is a convenient CLI wraper around Russula and can be used to launch
/// different protocols.
///
/// It currently supports launching server/client Netbench protocols.

#[derive(StructOpt, Debug)]
struct Opt {
    // Address for the Coordinator and Worker to communicate on.
    //
    // The Coordinator gets a list of workers addrs to 'connect' to.
    // The Worker gets its own addr to 'listen' on.
    #[structopt(long)]
    russula_port: u16,
    // FIXME replace russula_port with this.
    // workers get '0.0.0.0:port' and coord get 'x.x.x.x:port'; x can be 0 if testing locally
    // russula_pair_addr_list: Vec<SocketAddr>
    #[structopt(long, parse(try_from_str=parse_duration), default_value = "5s")]
    poll_delay: Duration,

    // TODO possibly move to Netbench Context
    #[structopt(long)]
    testing: bool,

    #[structopt(subcommand)]
    protocol: RussulaProtocol,
}

#[allow(clippy::enum_variant_names)]
#[derive(StructOpt, Debug)]
enum RussulaProtocol {
    NetbenchServerWorker {
        #[structopt(flatten)]
        ctx: netbench::ContextArgs,
    },
    NetbenchServerCoordinator {
        #[structopt(flatten)]
        ctx: netbench::ContextArgs,
    },
    NetbenchClientWorker {
        #[structopt(flatten)]
        ctx: netbench::ContextArgs,
    },
    NetbenchClientCoordinator {
        #[structopt(flatten)]
        ctx: netbench::ContextArgs,
    },
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> OrchResult<()> {
    let opt = Opt::from_args();

    let file_appender = tracing_appender::rolling::daily("./target", "russula.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(non_blocking)
        .init();

    debug!("{:?}", opt);
    println!("{:?}", opt);
    match &opt.protocol {
        RussulaProtocol::NetbenchServerWorker { ctx } => {
            let netbench_ctx = netbench::Context::new(opt.testing, ctx);
            run_server_worker(opt, netbench_ctx).await
        }
        RussulaProtocol::NetbenchServerCoordinator { ctx } => {
            let netbench_ctx = netbench::Context::new(opt.testing, ctx);
            run_server_coordinator(opt, netbench_ctx).await
        }
        RussulaProtocol::NetbenchClientWorker { ctx } => {
            let netbench_ctx = netbench::Context::new(opt.testing, ctx);
            run_client_worker(opt, netbench_ctx).await
        }
        RussulaProtocol::NetbenchClientCoordinator { ctx } => {
            let netbench_ctx = netbench::Context::new(opt.testing, ctx);
            run_client_coordinator(opt, netbench_ctx).await
        }
    };

    println!("cli done");
    Ok(())
}

async fn run_server_worker(opt: Opt, netbench_ctx: netbench::Context) {
    let id = 1;
    let protocol = server::WorkerProtocol::new(id, netbench_ctx);
    let worker = RussulaBuilder::new(
        BTreeSet::from_iter([russula_addr(opt.russula_port)]),
        protocol,
        opt.poll_delay,
    );
    let mut worker = worker.build().await.unwrap();
    worker.run_till_ready().await.unwrap();

    worker
        .run_till_state(server::WorkerState::Done)
        .await
        .unwrap();
}

async fn run_server_coordinator(opt: Opt, netbench_ctx: netbench::Context) {
    let protocol = server::CoordProtocol::new(netbench_ctx);
    let coord = RussulaBuilder::new(
        BTreeSet::from_iter([russula_addr(opt.russula_port)]),
        protocol,
        opt.poll_delay,
    );
    let mut coord = coord.build().await.unwrap();

    coord
        .run_till_state(server::CoordState::WorkersRunning)
        .await
        .unwrap();

    println!("Waiting for user input to continue ... WorkersRunning");
    let mut s = String::new();
    let _ = std::io::stdin().read_line(&mut s);
    println!("Continuing ... Running till Done");

    coord
        .run_till_state(server::CoordState::Done)
        .await
        .unwrap();
}

async fn run_client_worker(opt: Opt, netbench_ctx: netbench::Context) {
    let id = 1;
    let protocol = client::WorkerProtocol::new(id, netbench_ctx);
    let worker = RussulaBuilder::new(
        BTreeSet::from_iter([russula_addr(opt.russula_port)]),
        protocol,
        opt.poll_delay,
    );
    let mut worker = worker.build().await.unwrap();
    worker.run_till_ready().await.unwrap();

    worker
        .run_till_state(client::WorkerState::Done)
        .await
        .unwrap();
}

async fn run_client_coordinator(opt: Opt, netbench_ctx: netbench::Context) {
    let protocol = client::CoordProtocol::new(netbench_ctx);
    let coord = RussulaBuilder::new(
        BTreeSet::from_iter([russula_addr(opt.russula_port)]),
        protocol,
        opt.poll_delay,
    );
    let mut coord = coord.build().await.unwrap();

    coord
        .run_till_state(client::CoordState::WorkersRunning)
        .await
        .unwrap();

    coord
        .run_till_state(client::CoordState::Done)
        .await
        .unwrap();
}

// FIXME this only works for local runs.. coordinators attempting to connect to remote workers need
// an arg
fn russula_addr(port: u16) -> SocketAddr {
    format!("0.0.0.0:{}", port).parse().unwrap()
}
