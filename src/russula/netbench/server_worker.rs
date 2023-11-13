// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::{
    error::{RussulaError, RussulaResult},
    netbench::server_coord::CoordState,
    protocol::Protocol,
    StateApi, TransitionStep,
};
use async_trait::async_trait;
use core::fmt::Debug;
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, process::Command};
use sysinfo::{Pid, PidExt, ProcessExt, SystemExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum WorkerState {
    WaitCoordInit,
    Ready,
    Run,
    RunningAwaitKill(#[serde(skip)] u32),
    Stopped,
    Done,
}

#[derive(Clone, Copy)]
pub struct WorkerProtocol {
    id: u16,
    state: WorkerState,
}

impl WorkerProtocol {
    pub fn new(id: u16) -> Self {
        WorkerProtocol {
            id,
            state: WorkerState::WaitCoordInit,
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
        println!("{} listening on: {}", self.name(), addr);

        let (stream, _local_addr) = listener.accept().await.map_err(RussulaError::from)?;
        println!("{} success connection: {addr}", self.name());

        Ok(stream)
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
    fn name(&self) -> String {
        "[worker]".to_string()
    }

    async fn run(&mut self, stream: &TcpStream, name: String) -> RussulaResult<()> {
        match self {
            WorkerState::WaitCoordInit => {
                // self.notify_peer(stream).await?;
                self.await_next_msg(stream).await?;
            }
            WorkerState::Ready => {
                self.notify_peer(stream).await?;
                self.await_next_msg(stream).await?;
            }
            WorkerState::Run => {
                // some long task
                println!(
                    "{} some looooooooooooooooooooooooooooooooooooooooooooong task",
                    self.name()
                );
                let child = Command::new("sh")
                    .args(["sim_netbench_server.sh", &name])
                    .spawn()
                    .expect("Failed to start echo process");

                let pid = child.id();
                println!(
                    "{}----------------------------child id {}",
                    self.name(),
                    pid
                );

                *self = WorkerState::RunningAwaitKill(pid);
            }
            WorkerState::RunningAwaitKill(pid) => {
                let pid = *pid;
                self.notify_peer(stream).await?;
                self.await_action_msg(stream).await?;

                let pid = Pid::from_u32(pid);
                let mut system = sysinfo::System::new();
                if system.refresh_process(pid) {
                    let process = system.process(pid).unwrap();
                    let kill = process.kill();
                    println!("did KILL pid: {} {}----------------------------", pid, kill);
                }

                // FIXME fix this
                self.transition_self_or_user_driven(stream).await?;
            }
            WorkerState::Stopped => {
                self.notify_peer(stream).await?;
                self.await_next_msg(stream).await?;
            }
            WorkerState::Done => {
                self.notify_peer(stream).await?;
            }
        }
        Ok(())
    }

    fn transition_step(&self) -> TransitionStep {
        match self {
            WorkerState::WaitCoordInit => {
                TransitionStep::AwaitNext(CoordState::CheckWorker.as_bytes())
            }
            WorkerState::Ready => TransitionStep::AwaitNext(CoordState::RunWorker.as_bytes()),
            WorkerState::Run => TransitionStep::SelfDriven,
            WorkerState::RunningAwaitKill(_) => {
                TransitionStep::AwaitAction(CoordState::KillWorker.as_bytes())
            }
            WorkerState::Stopped => TransitionStep::AwaitNext(CoordState::Done.as_bytes()),
            WorkerState::Done => TransitionStep::Finished,
        }
    }

    fn next_state(&self) -> Self {
        match self {
            WorkerState::WaitCoordInit => WorkerState::Ready,
            WorkerState::Ready => WorkerState::Run,
            // FIXME error prone
            WorkerState::Run => WorkerState::RunningAwaitKill(0),
            WorkerState::RunningAwaitKill(_) => WorkerState::Stopped,
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
