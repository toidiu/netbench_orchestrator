// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

mod client_coord;
mod client_worker;
mod server_coord;
mod server_worker;

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

pub mod client {
    pub use super::{client_coord::*, client_worker::*};
}
