// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

mod error;
mod russula;

use error::{OrchError, OrchResult};
use structopt::{clap::arg_enum, StructOpt};

#[derive(StructOpt)]
struct Opt {
    /// specify the protocol
    #[structopt(possible_values = &RussulaProtocol::variants(), case_insensitive = true, long, short)]
    protocol: RussulaProtocol,
}

arg_enum! {
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

    tracing_subscriber::fmt::init();

    println!("hi");
    Ok(())
}
