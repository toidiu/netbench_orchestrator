// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::{
    error::{RussulaError, RussulaResult},
    netbench::client::CoordState,
    network_utils::Msg,
    protocol::Protocol,
    StateApi, TransitionStep,
};
use async_trait::async_trait;
use core::fmt::Debug;
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, process::Command};
use sysinfo::{Pid, PidExt, SystemExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, info};

// Only used when creating a state variant
const PLACEHOLDER_PID: u32 = 1000;

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum WorkerState {
    WaitCoordInit,
    Ready,
    Run,
    Running(#[serde(skip)] u32),
    RunningAwaitComplete(#[serde(skip)] u32),
    Stopped,
    Done,
}

#[derive(Clone, Copy)]
pub struct WorkerProtocol {
    id: u16,
    state: WorkerState,
    coord_state: CoordState,
}

impl WorkerProtocol {
    pub fn new(id: u16) -> Self {
        WorkerProtocol {
            id,
            state: WorkerState::WaitCoordInit,
            coord_state: CoordState::CheckWorker,
        }
    }
}

#[async_trait]
impl Protocol for WorkerProtocol {
    type State = WorkerState;

    fn name(&self) -> String {
        format!("[client-worker-{}]", self.id)
    }

    async fn connect(&self, addr: &SocketAddr) -> RussulaResult<TcpStream> {
        let listener = TcpListener::bind(addr).await.unwrap();
        info!("{} listening on: {}", self.name(), addr);

        let (stream, _local_addr) = listener.accept().await.map_err(RussulaError::from)?;
        info!("{} success connection: {addr}", self.name());

        Ok(stream)
    }

    fn update_peer_state(&mut self, msg: Msg) -> RussulaResult<()> {
        self.coord_state = CoordState::from_msg(msg)?;
        debug!(
            "{} ................................................................. {:?}",
            self.name(),
            self.coord_state
        );

        Ok(())
    }

    fn state(&self) -> &Self::State {
        &self.state
    }

    fn state_mut(&mut self) -> &mut Self::State {
        &mut self.state
    }

    fn ready_state(&self) -> Self::State {
        WorkerState::Ready
    }

    fn done_state(&self) -> Self::State {
        WorkerState::Done
    }

    async fn run(&mut self, stream: &TcpStream) -> RussulaResult<Option<Msg>> {
        match self.state_mut() {
            WorkerState::WaitCoordInit => {
                // self.notify_peer(stream).await?;
                self.await_next_msg(stream).await.map(Some)
            }
            WorkerState::Ready => {
                self.state().notify_peer(stream).await?;
                self.await_next_msg(stream).await.map(Some)
            }
            WorkerState::Run => {
                // some long task
                debug!("{} starting some task sim_netbench_client", self.name());
                cfg_if::cfg_if! {
                    // simulate the netbench process for testing
                    if #[cfg(test)] {
                        let child = Command::new("sh")
                            .args(["sim_netbench_client.sh", "bla"])
                            .spawn()
                            .expect("Failed to start sim_netbench_client process");
                    } else {
                        // FIXME do this
                        let child = Command::new("sh")
                            .args(["sim_netbench_client.sh", "bla"])
                            .spawn()
                            .expect("Failed to start blaaa process");
                    }
                };
                // SCENARIO=./target/netbench/connect.json SERVER_0=localhost:4433 ./target/release/netbench-driver-s2n-quic-client ./target/netbench/connect.json
                // let bla = Command::new("/home/ec2-user/bin/netbench-driver-s2n-quic-client")
                //     .env("SCENARIO", "/home/ec2-user/request_response.json")
                //     // FIXME get ip
                //     .env("SERVER_0", "xxx:9000")
                //     .args(["/home/ec2-user/request_response.json"])
                //     .spawn()
                //     .expect("Failed to start netbench-driver-s2n-quic-client process");

                let pid = child.id();
                debug!(
                    "{}----------------------------child id {}",
                    self.name(),
                    pid
                );

                *self.state_mut() = WorkerState::Running(pid);
                Ok(None)
            }
            WorkerState::Running(_pid) => {
                self.state().notify_peer(stream).await?;
                self.await_next_msg(stream).await.map(Some)
            }
            WorkerState::RunningAwaitComplete(pid) => {
                let pid = *pid;
                self.state().notify_peer(stream).await?;

                let pid = Pid::from_u32(pid);
                let mut system = sysinfo::System::new();

                let is_process_complete = !system.refresh_process(pid);

                if is_process_complete {
                    debug!(
                        "process COMPLETED! pid: {} ----------------------------",
                        pid
                    );
                } else {
                    debug!(
                        "process still RUNNING! pid: {} ----------------------------",
                        pid
                    );
                }

                // FIXME fix this
                self.state_mut()
                    .transition_self_or_user_driven(stream)
                    .await?;
                Ok(None)
            }
            WorkerState::Stopped => {
                self.state().notify_peer(stream).await?;
                self.await_next_msg(stream).await.map(Some)
            }
            WorkerState::Done => {
                self.state().notify_peer(stream).await?;
                Ok(None)
            }
        }
    }
}

#[async_trait]
impl StateApi for WorkerState {
    fn name_prefix(&self) -> String {
        "client-worker".to_string()
    }

    async fn run(&mut self, stream: &TcpStream, name: String) -> RussulaResult<Option<Msg>> {
        match self {
            WorkerState::WaitCoordInit => {
                // self.notify_peer(stream).await?;
                self.await_next_msg(stream).await.map(Some)
            }
            WorkerState::Ready => {
                self.notify_peer(stream).await?;
                self.await_next_msg(stream).await.map(Some)
            }
            WorkerState::Run => {
                // some long task
                debug!(
                    "{} starting some task sim_netbench_client",
                    self.name(stream)
                );
                cfg_if::cfg_if! {
                    // simulate the netbench process for testing
                    if #[cfg(test)] {
                        let child = Command::new("sh")
                            .args(["sim_netbench_client.sh", &name])
                            .spawn()
                            .expect("Failed to start sim_netbench_client process");
                    } else {
                        // FIXME do this
                        let child = Command::new("sh")
                            .args(["sim_netbench_client.sh", &name])
                            .spawn()
                            .expect("Failed to start blaaa process");
                    }
                };
                // // SCENARIO=./target/netbench/connect.json SERVER_0=localhost:4433 ./target/release/netbench-driver-s2n-quic-client ./target/netbench/connect.json
                // let bla = Command::new("/home/ec2-user/bin/netbench-driver-s2n-quic-client")
                //     .env("SCENARIO", "/home/ec2-user/request_response.json")
                //     // FIXME get ip
                //     .env("SERVER_0", "xxx:9000")
                //     .args(["/home/ec2-user/request_response.json"])
                //     .spawn()
                //     .expect("Failed to start netbench-driver-s2n-quic-client process");

                let pid = child.id();
                debug!(
                    "{}----------------------------child id {}",
                    self.name(stream),
                    pid
                );

                *self = WorkerState::Running(pid);
                Ok(None)
            }
            WorkerState::Running(_pid) => {
                self.notify_peer(stream).await?;
                self.await_next_msg(stream).await.map(Some)
            }
            WorkerState::RunningAwaitComplete(pid) => {
                let pid = *pid;
                self.notify_peer(stream).await?;

                let pid = Pid::from_u32(pid);
                let mut system = sysinfo::System::new();

                let is_process_complete = !system.refresh_process(pid);

                if is_process_complete {
                    debug!(
                        "process COMPLETED! pid: {} ----------------------------",
                        pid
                    );
                } else {
                    debug!(
                        "process still RUNNING! pid: {} ----------------------------",
                        pid
                    );
                }

                // FIXME fix this
                self.transition_self_or_user_driven(stream).await?;
                Ok(None)
            }
            WorkerState::Stopped => {
                self.notify_peer(stream).await?;
                self.await_next_msg(stream).await.map(Some)
            }
            WorkerState::Done => {
                self.notify_peer(stream).await?;
                Ok(None)
            }
        }
    }

    fn transition_step(&self) -> TransitionStep {
        match self {
            WorkerState::WaitCoordInit => {
                TransitionStep::AwaitNext(CoordState::CheckWorker.as_bytes())
            }
            WorkerState::Ready => TransitionStep::AwaitNext(CoordState::RunWorker.as_bytes()),
            WorkerState::Run => TransitionStep::SelfDriven,
            WorkerState::Running(_) => {
                TransitionStep::AwaitNext(CoordState::WorkersRunning.as_bytes())
            }
            WorkerState::RunningAwaitComplete(_) => TransitionStep::SelfDriven,
            WorkerState::Stopped => TransitionStep::AwaitNext(CoordState::Done.as_bytes()),
            WorkerState::Done => TransitionStep::Finished,
        }
    }

    fn next_state(&self) -> Self {
        match self {
            WorkerState::WaitCoordInit => WorkerState::Ready,
            WorkerState::Ready => WorkerState::Run,
            // FIXME error prone
            WorkerState::Run => WorkerState::Running(PLACEHOLDER_PID),
            WorkerState::Running(pid) => WorkerState::RunningAwaitComplete(*pid),
            WorkerState::RunningAwaitComplete(_) => WorkerState::Stopped,
            WorkerState::Stopped => WorkerState::Done,
            WorkerState::Done => WorkerState::Done,
        }
    }
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn netbench_state() {}
}
