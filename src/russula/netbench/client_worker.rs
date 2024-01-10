// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::Context;
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
use sysinfo::{Pid, PidExt, ProcessExt, SystemExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, info, warn};

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
    netbench_ctx: Context,
}

impl WorkerProtocol {
    pub fn new(id: u16, netbench_ctx: Context) -> Self {
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
        format!("client-worker-{}", self.id)
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
                let child = match false {
                    false => {
                        let out_log_file = "client.json";
                        let output_json = File::create(out_log_file).expect("failed to open log");

                        // SCENARIO=./target/netbench/connect.json SERVER_0=localhost:4433
                        //   ./target/release/netbench-driver-s2n-quic-client ./target/netbench/connect.json
                        info!("{} run netbench process", self.name());
                        println!("{} run netbench process", self.name());

                        let netbench_path = self.netbench_ctx.netbench_path.to_str().unwrap();
                        let collector = format!("{}/netbench-collector", netbench_path);
                        // driver value ex.: netbench-driver-s2n-quic-client
                        let driver = format!("{}/{}", netbench_path, self.netbench_ctx.driver);
                        let scenario = format!("{}/{}", netbench_path, self.netbench_ctx.scenario);

                        let mut cmd = Command::new(collector);
                        // FIXME update Netbench to take a list of Server IP
                        let server_addr = self.netbench_ctx.peer_list.first().unwrap();
                        cmd.env("SERVER_0", server_addr.to_string())
                            .args([&driver, "--scenario", &scenario])
                            .stdout(output_json);
                        println!("{:?}", cmd);
                        cmd.spawn()
                            .expect("Failed to start netbench client process")
                    }
                    true => {
                        info!("{} run sim_netbench_client", self.name());
                        Command::new("sh")
                            .args(["sim_netbench_client.sh", &self.name()])
                            .spawn()
                            .expect("Failed to start sim_netbench_client process")
                    }
                };

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
                let mut system = sysinfo::System::new_all();

                let process = system.process(pid);
                // let is_process_complete = !system.refresh_process(pid);

                if let Some(process) = process {
                    debug!(
                        "process still RUNNING! pid: {} status: {:?} ----------------------------",
                        process.pid(),
                        process.status()
                    );
                    // FIXME somethings is causing the collector to become a Zombie process.
                    //
                    // We can detect the zombie process and continue because this is a testing
                    // utility but we should come back and fix this
                    // - Collector issue
                    // - how Russula calls Collector issue
                    // - Command::stdout file issue
                    //
                    // root       54245  Sl ./target/debug/russula_cli --protocol NetbenchClientWorker --port 9000 --peer-list 54.198.168.151:4433
                    // root       54688  Z  [netbench-collec] <defunct>

                    if let sysinfo::ProcessStatus::Zombie = process.status() {
                        warn!(
                            "Process pid: {} is a Zombie.. ignoring and continuing",
                            process.pid()
                        );
                        self.state_mut()
                            .transition_self_or_user_driven(stream)
                            .await?;
                    }
                } else {
                    info!(
                        "Process COMPLETED! pid: {} ----------------------------",
                        pid
                    );
                    // FIXME get this
                    // format!("aws s3 cp client.json {}/results/request_response/s2n-quic", STATE.s3_path(unique_id));
                    // Command::new("sh")
                    //     .args([])
                    //     .spawn()
                    //     .expect("Upload netbench result")
                    //     .wait_with_output();

                    self.state_mut()
                        .transition_self_or_user_driven(stream)
                        .await?;
                }

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
