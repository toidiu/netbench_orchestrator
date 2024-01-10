// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use std::{net::SocketAddr, path::PathBuf};
use structopt::{clap::arg_enum, StructOpt};

mod client_coord;
mod client_worker;
mod server_coord;
mod server_worker;

#[derive(StructOpt, Debug)]
pub struct ContextArgs {
    // The path to the netbench utility and scenario file.
    #[structopt(long, default_value = "/home/ec2-user/bin")]
    netbench_path: PathBuf,

    #[structopt(long)]
    driver: String,

    // The name of the scenario file.
    //
    // https://github.com/aws/s2n-netbench/tree/main/netbench-scenarios
    #[structopt(long, default_value = "request_response.json")]
    scenario: String,

    // The list of Client and Server peers
    #[structopt(long)]
    pub peer_list: Vec<SocketAddr>,
}

impl ContextArgs {
    // FIXME directly create Context
    pub fn for_russula_coordinator(driver_name: &str) -> Self {
        Self {
            netbench_path: "unused_by_coordinator".into(),
            driver: driver_name.into(),
            scenario: "unused_by_coordinator".to_owned(),
            peer_list: vec![],
        }
    }
}

#[derive(Debug, Clone)]
pub struct Context {
    pub peer_list: Vec<SocketAddr>,
    netbench_path: PathBuf,
    driver: String,
    scenario: String,
    testing: bool,
}

impl Context {
    pub fn new(testing: bool, ctx: &ContextArgs) -> Self {
        Context {
            peer_list: ctx.peer_list.clone(),
            netbench_path: ctx.netbench_path.clone(),
            driver: ctx.driver.clone(),
            scenario: ctx.scenario.clone(),
            testing,
        }
    }

    #[cfg(test)]
    pub fn testing() -> Self {
        Context {
            peer_list: vec![],
            netbench_path: "".into(),
            driver: "".to_string(),
            scenario: "".to_string(),
            testing: true,
        }
    }
}

// CheckWorker   --------->  WaitCoordInit
//                              |
//                              v
// CheckWorker   <---------  Ready
//    |
//    v
// Ready
//    | (user)
//    v
// RunWorker     --------->  Ready
//                              |
//                              v
//                           Run
//                              | (self)
//                              v
// RunWorker     <---------  RunningAwaitKill
//    |
//    v
// WorkersRunning
//    | (user)
//    v
// KillWorker    --------->  RunningAwaitKill
//                              |
//                              v
//                           Killing
//                              | (self)
//                              v
// WorkerKilled  <---------  Stopped
//    |
//    v
// Done          --------->  Stopped
//                              |
//                              v
//                           Done
pub mod server {
    pub use super::{server_coord::*, server_worker::*};
}

// CheckWorker   --------->  WaitCoordInit
//                              |
//                              v
// CheckWorker   <---------  Ready
//    |
//    v
// Ready
//    | (user)
//    v
// RunWorker     --------->  Ready
//                              |
//                              v
//                           Run
//                              | (self)
//                              v
// RunWorker     <---------  Running
//    |
//    v
// WorkersRunning ---------> Running
//                              |
//                              v
//                           RunningAwaitComplete
//                              | (self)
//                              v
// WorkersRunning <---------  Stopped
//    |
//    v
// Done          --------->  Stopped
//                              |
//                              v
//                           Done
pub mod client {
    pub use super::{client_coord::*, client_worker::*};
}
