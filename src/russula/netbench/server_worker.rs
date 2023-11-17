// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

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
use std::{net::SocketAddr, process::Command};
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
        format!("[worker-{}]", self.id)
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

    fn state_ready(&self) -> Self::State {
        WorkerState::Ready
    }
}

#[async_trait]
impl StateApi for WorkerState {
    fn name_prefix(&self) -> String {
        "worker".to_string()
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
                println!(
                    "{} starting some task sim_netbench_server",
                    self.name(stream)
                );
                let child = Command::new("sh")
                    .args(["sim_netbench_server.sh", &name])
                    .spawn()
                    .expect("Failed to start echo process");

                let pid = child.id();
                println!(
                    "{}----------------------------child id {}",
                    self.name(stream),
                    pid
                );

                *self = WorkerState::RunningAwaitKill(pid);
                Ok(None)
            }
            WorkerState::RunningAwaitKill(_pid) => {
                self.notify_peer(stream).await?;
                self.await_next_msg(stream).await.map(Some)
            }
            WorkerState::Killing(pid) => {
                let pid = Pid::from_u32(*pid);
                let mut system = sysinfo::System::new();
                if system.refresh_process(pid) {
                    let process = system.process(pid).unwrap();
                    let kill = process.kill();
                    println!("did KILL pid: {} {}----------------------------", pid, kill);
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
