// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::ServerContext;
use crate::russula::{
    error::{RussulaError, RussulaResult},
    netbench::server_coord::CoordState,
    network_utils::Msg,
    protocol::Protocol,
    StateApi, TransitionStep,
};
use async_trait::async_trait;
use core::fmt::Debug;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    net::SocketAddr,
    process::{Command, Stdio},
};
use sysinfo::{Pid, PidExt, ProcessExt, SystemExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, info};

// Only used when creating a state variant
const PLACEHOLDER_PID: u32 = 1000;

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum WorkerState {
    WaitCoordInit,
    Ready,
    Run,
    RunningAwaitKill(#[serde(skip)] u32),
    Killing(#[serde(skip)] u32),
    Stopped,
    Done,
}

#[derive(Clone, Debug)]
pub struct WorkerProtocol {
    id: u16,
    state: WorkerState,
    coord_state: CoordState,
    netbench_ctx: ServerContext,
}

impl WorkerProtocol {
    pub fn new(id: u16, netbench_ctx: ServerContext) -> Self {
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
        format!("server-worker-{}", self.id)
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
                // self.notify_peer(stream).await?;
                self.await_next_msg(stream).await.map(Some)
            }
            WorkerState::Ready => {
                self.state().notify_peer(stream).await?;
                self.await_next_msg(stream).await.map(Some)
            }
            WorkerState::Run => {
                let child = match &self.netbench_ctx.testing {
                    false => {
                        // TODO use unique id
                        let out_log_file = "net_data_server.json";
                        let out_log_file = format!("net_data_{}.json", self.name());
                        let output_json = File::create(out_log_file).expect("failed to open log");

                        // sudo SCENARIO=./target/netbench/connect.json ./target/release/netbench-collector
                        //   ./target/release/netbench-driver-s2n-quic-server
                        info!("{} run task netbench", self.state().name(stream));
                        println!("{} run task netbench", self.state().name(stream));

                        let netbench_path = self.netbench_ctx.netbench_path.to_str().unwrap();
                        let collector = format!("{}/s2n-netbench-collector", netbench_path);
                        // driver value ex.: netbench-driver-s2n-quic-server
                        let driver = format!("{}/{}", netbench_path, self.netbench_ctx.driver);
                        let scenario = format!("{}/{}", netbench_path, self.netbench_ctx.scenario);

                        debug!("netbench_port: {}", self.netbench_ctx.netbench_port);

                        let mut cmd = Command::new(collector);
                        cmd.env("PORT", self.netbench_ctx.netbench_port.to_string());
                        cmd.arg("--disable-bpf");
                        cmd.args([&driver, "--scenario", &scenario])
                            .stdout(output_json);
                        println!("{:?}", cmd);
                        debug!("{:?}", cmd);
                        cmd.spawn()
                            .expect("Failed to start netbench server process")
                    }
                    true => {
                        info!("{} run task sim_netbench_server", self.state().name(stream));
                        Command::new("sh")
                            .args(["scripts/sim_netbench_server.sh", &self.name()])
                            .spawn()
                            .expect("Failed to start echo process")
                    }
                };

                let pid = child.id();
                debug!(
                    "{}----------------------------child id {}",
                    self.state().name(stream),
                    pid
                );

                *self.state_mut() = WorkerState::RunningAwaitKill(pid);
                Ok(None)
            }
            WorkerState::RunningAwaitKill(_pid) => {
                self.state().notify_peer(stream).await?;
                self.await_next_msg(stream).await.map(Some)
            }
            WorkerState::Killing(pid) => {
                let pid = Pid::from_u32(*pid);
                let mut system = sysinfo::System::new_all();
                if system.refresh_process(pid) {
                    let process = system.process(pid).unwrap();
                    let kill = process.kill();
                    debug!("did KILL pid: {} {}----------------------------", pid, kill);
                }

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
        "server-worker".to_string()
    }

    fn transition_step(&self) -> TransitionStep {
        match self {
            WorkerState::WaitCoordInit => {
                TransitionStep::AwaitNext(CoordState::CheckWorker.as_bytes())
            }
            WorkerState::Ready => TransitionStep::AwaitNext(CoordState::RunWorker.as_bytes()),
            WorkerState::Run => TransitionStep::SelfDriven,
            WorkerState::RunningAwaitKill(_) => {
                TransitionStep::AwaitNext(CoordState::KillWorker.as_bytes())
            }
            WorkerState::Killing(_) => TransitionStep::SelfDriven,
            WorkerState::Stopped => TransitionStep::AwaitNext(CoordState::Done.as_bytes()),
            WorkerState::Done => TransitionStep::Finished,
        }
    }

    fn next_state(&self) -> Self {
        match self {
            WorkerState::WaitCoordInit => WorkerState::Ready,
            WorkerState::Ready => WorkerState::Run,
            // FIXME error prone
            WorkerState::Run => WorkerState::RunningAwaitKill(PLACEHOLDER_PID),
            WorkerState::RunningAwaitKill(pid) => WorkerState::Killing(*pid),
            WorkerState::Killing(_) => WorkerState::Stopped,
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
