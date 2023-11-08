// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::{
    error::{RussulaError, RussulaResult},
    netbench_server_coord::CoordNetbenchServerState,
    protocol::Protocol,
    StateApi, TransitionStep,
};
use async_trait::async_trait;
use std::{net::SocketAddr, process::Command};
use sysinfo::{Pid, PidExt, ProcessExt, SystemExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Copy, Clone, Debug)]
pub enum WorkerNetbenchServerState {
    WaitPeerInit,
    Ready,
    Run,
    RunningAwaitPeer(u32),
    Done,
}

#[derive(Clone, Copy)]
pub struct NetbenchWorkerServerProtocol {
    id: u16,
    state: WorkerNetbenchServerState,
}

impl NetbenchWorkerServerProtocol {
    pub fn new(id: u16) -> Self {
        NetbenchWorkerServerProtocol {
            id,
            state: WorkerNetbenchServerState::WaitPeerInit,
        }
    }
}

#[async_trait]
impl Protocol for NetbenchWorkerServerProtocol {
    type State = WorkerNetbenchServerState;

    fn name(&self) -> String {
        format!("[worker-{}]", self.id)
    }

    async fn connect(&self, addr: &SocketAddr) -> RussulaResult<TcpStream> {
        let listener = TcpListener::bind(addr).await.unwrap();
        println!("{} listening on: {}", self.name(), addr);

        let (stream, _local_addr) =
            listener
                .accept()
                .await
                .map_err(|err| RussulaError::NetworkFail {
                    dbg: err.to_string(),
                })?;
        println!("{} success connection: {addr}", self.name());

        Ok(stream)
    }

    async fn run_till_ready(&mut self, stream: &TcpStream) -> RussulaResult<()> {
        self.run_till_state(stream, WorkerNetbenchServerState::Ready)
            .await
    }

    fn state(&self) -> &Self::State {
        &self.state
    }

    fn state_mut(&mut self) -> &mut Self::State {
        &mut self.state
    }
}

#[async_trait]
impl StateApi for WorkerNetbenchServerState {
    fn name(&self) -> String {
        "[worker]".to_string()
    }

    async fn run(&mut self, stream: &TcpStream, name: String) -> RussulaResult<()> {
        match self {
            WorkerNetbenchServerState::WaitPeerInit => self.await_peer_msg(stream).await,
            WorkerNetbenchServerState::Ready => self.await_peer_msg(stream).await,
            WorkerNetbenchServerState::Run => {
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
                *self = WorkerNetbenchServerState::RunningAwaitPeer(pid);
                self.notify_peer(stream).await.map(|_| ())
            }
            WorkerNetbenchServerState::RunningAwaitPeer(pid) => {
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
            WorkerNetbenchServerState::Done => self.transition_next(stream).await.map(|_| ()),
        }
    }

    fn transition_step(&self) -> TransitionStep {
        match self {
            WorkerNetbenchServerState::WaitPeerInit => {
                TransitionStep::AwaitPeer(CoordNetbenchServerState::CheckPeer.as_bytes())
            }
            WorkerNetbenchServerState::Ready => {
                TransitionStep::AwaitPeer(CoordNetbenchServerState::RunPeer.as_bytes())
            }
            WorkerNetbenchServerState::Run => TransitionStep::SelfDriven,
            WorkerNetbenchServerState::RunningAwaitPeer(_) => {
                TransitionStep::AwaitPeer(CoordNetbenchServerState::KillPeer.as_bytes())
            }
            WorkerNetbenchServerState::Done => TransitionStep::Finished,
        }
    }

    fn next_state(&self) -> Self {
        match self {
            WorkerNetbenchServerState::WaitPeerInit => WorkerNetbenchServerState::Ready,
            WorkerNetbenchServerState::Ready => WorkerNetbenchServerState::Run,
            // FIXME error prone
            WorkerNetbenchServerState::Run => WorkerNetbenchServerState::RunningAwaitPeer(0),
            WorkerNetbenchServerState::RunningAwaitPeer(_) => WorkerNetbenchServerState::Done,
            WorkerNetbenchServerState::Done => WorkerNetbenchServerState::Done,
        }
    }

    fn eq(&self, other: &Self) -> bool {
        match self {
            WorkerNetbenchServerState::WaitPeerInit => {
                matches!(other, WorkerNetbenchServerState::WaitPeerInit)
            }
            WorkerNetbenchServerState::Ready => {
                matches!(other, WorkerNetbenchServerState::Ready)
            }
            WorkerNetbenchServerState::Run => {
                matches!(other, WorkerNetbenchServerState::Run)
            }
            WorkerNetbenchServerState::RunningAwaitPeer(_pid) => {
                matches!(other, WorkerNetbenchServerState::RunningAwaitPeer(_))
            }
            WorkerNetbenchServerState::Done => {
                matches!(other, WorkerNetbenchServerState::Done)
            }
        }
    }

    fn as_bytes(&self) -> &'static [u8] {
        match self {
            WorkerNetbenchServerState::WaitPeerInit => b"server_wait_peer_init",
            WorkerNetbenchServerState::Ready => b"server_ready",
            WorkerNetbenchServerState::Run => b"server_wait_peer_done",
            WorkerNetbenchServerState::RunningAwaitPeer(_) => b"server_running_await_peer",
            WorkerNetbenchServerState::Done => b"server_done",
        }
    }

    fn from_bytes(bytes: &[u8]) -> RussulaResult<Self> {
        let state = match bytes {
            b"server_wait_peer_init" => WorkerNetbenchServerState::WaitPeerInit,
            b"server_ready" => WorkerNetbenchServerState::Ready,
            b"server_wait_peer_done" => WorkerNetbenchServerState::Run,
            b"server_done" => WorkerNetbenchServerState::Done,
            bad_msg => {
                return Err(RussulaError::BadMsg {
                    dbg: format!("unrecognized msg {:?}", std::str::from_utf8(bad_msg)),
                })
            }
        };

        Ok(state)
    }
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn netbench_state() {}
}
