// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::PeerList;
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
use std::{fs::File, net::SocketAddr, process::Command};
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

#[derive(Clone)]
pub struct WorkerProtocol {
    id: u16,
    state: WorkerState,
    coord_state: CoordState,
    netbench_ctx: Option<PeerList>,
}

impl WorkerProtocol {
    pub fn new(id: u16, netbench_ctx: Option<PeerList>) -> Self {
        WorkerProtocol {
            id,
            state: WorkerState::WaitCoordInit,
            coord_state: CoordState::CheckWorker,
            netbench_ctx,
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
        debug!("{} ... peer_state {:?}", self.name(), self.coord_state);

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
                // self.state().notify_peer(stream).await?;
                self.await_next_msg(stream).await.map(Some)
            }
            WorkerState::Ready => {
                self.state().notify_peer(stream).await?;
                self.await_next_msg(stream).await.map(Some)
            }
            WorkerState::Run => {
                let child = match &self.netbench_ctx {
                    Some(ctx) => {
                        // SCENARIO=./target/netbench/connect.json SERVER_0=localhost:4433
                        //   ./target/release/netbench-driver-s2n-quic-client ./target/netbench/connect.json
                        info!("{} RUN NETBENCH PROCESS", self.name());
                        let peer_sock_addr = ctx.0.get(0).expect("get the first peer sock_addr");

                        // FIXME figure out different way for local and remote
                        // remote runs
                        let collector = "/home/ec2-user/bin/netbench-collector";
                        let driver = "/home/ec2-user/bin/netbench-driver-s2n-quic-client";
                        let scenario = "/home/ec2-user/request_response.json";
                        // local testing
                        let collector = "netbench-collector";
                        let driver = "netbench-driver-s2n-quic-client";
                        let scenario = "request_response.json";

                        let out_json =
                            format!("netbench-client-{}.json", self.state().name(stream));
                        let output_json = File::create(out_json).expect("failed to open log");
                        let mut cmd = Command::new(collector);
                        cmd.env("SERVER_0", peer_sock_addr.to_string())
                            .args([driver, "--scenario", scenario])
                            .stdout(output_json);

                        println!("{:?}", cmd);
                        cmd.spawn()
                            .expect("Failed to start netbench-driver-s2n-quic-client process")
                    }
                    None => {
                        info!("{} RUN SIM_NETBENCH_CLIENT", self.name());
                        Command::new("sh")
                            .args(["sim_netbench_client.sh", &self.name()])
                            .spawn()
                            .expect("Failed to start sim_netbench_client process")
                    }
                };

                println!("-----------{:?}", child);

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
