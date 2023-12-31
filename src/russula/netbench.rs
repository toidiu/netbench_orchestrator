// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use std::net::SocketAddr;
use structopt::{clap::arg_enum, StructOpt};

mod client_coord;
mod client_worker;
mod server_coord;
mod server_worker;

#[derive(Debug, Clone)]
pub struct PeerList(Vec<SocketAddr>);

impl std::str::FromStr for PeerList {
    type Err = Box<dyn std::error::Error>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(PeerList(
            s.split(',')
                .map(|x| {
                    let str_value = x.trim();
                    SocketAddr::from_str(str_value).unwrap()
                })
                .collect(),
        ))
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
