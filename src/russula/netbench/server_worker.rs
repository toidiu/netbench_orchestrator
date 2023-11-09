// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::{
    error::{RussulaError, RussulaResult},
    netbench::server_coord::CoordState,
    protocol::Protocol,
    StateApi, TransitionStep,
};
use async_trait::async_trait;
use core::{fmt::Debug, task::Poll};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, process::Command};
use sysinfo::{Pid, PidExt, ProcessExt, SystemExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum WorkerState {
    WaitPeerInit,
    Ready,
    Run,
    RunningAwaitPeer(#[serde(skip)] u32),
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
            state: WorkerState::WaitPeerInit,
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

    async fn poll_ready(&mut self, stream: &TcpStream) -> RussulaResult<Poll<()>> {
        self.poll_state(stream, WorkerState::Ready).await
    }

    fn state(&self) -> &Self::State {
        &self.state
    }

    fn state_mut(&mut self) -> &mut Self::State {
        &mut self.state
    }
}

#[async_trait]
impl StateApi for WorkerState {
    fn name(&self) -> String {
        "[worker]".to_string()
    }

    async fn run(&mut self, stream: &TcpStream, name: String) -> RussulaResult<()> {
        match self {
            WorkerState::WaitPeerInit => self.await_peer_msg(stream).await,
            WorkerState::Ready => self.await_peer_msg(stream).await,
            WorkerState::Run => {
                // some long task
                println!(
                    "{} some looooooooooooooooooooooooooooooooooooooooooooong task",
                    self.name()
                );
                let child = Command::new("sh")
                    .args(["long_running_process.sh", &name])
                    .spawn()
                    .expect("Failed to start echo process");

                let pid = child.id();
                println!(
                    "{}----------------------------child id {}",
                    self.name(),
                    pid
                );

                // FIXME error prone.. see next_state()
                *self = WorkerState::RunningAwaitPeer(pid);
                self.notify_peer(stream).await.map(|_| ())
            }
            WorkerState::RunningAwaitPeer(pid) => {
                let pid = *pid;
                self.await_peer_msg(stream).await?;

                let pid = Pid::from_u32(pid);
                let mut system = sysinfo::System::new();
                if system.refresh_process(pid) {
                    let process = system.process(pid).unwrap();
                    let kill = process.kill();
                    println!("did KILL pid: {} {}----------------------------", pid, kill);
                }

                self.transition_next(stream).await.map(|_| ())
            }
            WorkerState::Done => self.transition_next(stream).await.map(|_| ()),
        }
    }

    fn transition_step(&self) -> TransitionStep {
        match self {
            WorkerState::WaitPeerInit => {
                TransitionStep::AwaitPeer(CoordState::CheckPeer.as_bytes())
            }
            WorkerState::Ready => TransitionStep::AwaitPeer(CoordState::RunPeer.as_bytes()),
            WorkerState::Run => TransitionStep::SelfDriven,
            WorkerState::RunningAwaitPeer(_) => {
                TransitionStep::AwaitPeer(CoordState::KillPeer.as_bytes())
            }
            WorkerState::Done => TransitionStep::Finished,
        }
    }

    fn next_state(&self) -> Self {
        match self {
            WorkerState::WaitPeerInit => WorkerState::Ready,
            WorkerState::Ready => WorkerState::Run,
            // FIXME error prone
            WorkerState::Run => WorkerState::RunningAwaitPeer(0),
            WorkerState::RunningAwaitPeer(_) => WorkerState::Done,
            WorkerState::Done => WorkerState::Done,
        }
    }
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn netbench_state() {}
}