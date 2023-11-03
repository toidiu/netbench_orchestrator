// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::netbench_server_coord::CoordNetbenchServerState;
use crate::russula::network_utils;
use crate::russula::StateApi;
use crate::russula::TransitionStep;
use async_trait::async_trait;
use bytes::Bytes;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};

use crate::russula::error::{RussulaError, RussulaResult};
use crate::russula::protocol::Protocol;

#[derive(Copy, Clone, Debug)]
pub enum WorkerNetbenchServerState {
    WaitPeerInit,
    Ready,
    Run,
    Done,
}

#[derive(Clone, Copy)]
pub struct NetbenchWorkerServerProtocol {
    state: WorkerNetbenchServerState,
}

impl NetbenchWorkerServerProtocol {
    pub fn new() -> Self {
        NetbenchWorkerServerProtocol {
            state: WorkerNetbenchServerState::WaitPeerInit,
        }
    }
}

#[async_trait]
impl Protocol for NetbenchWorkerServerProtocol {
    type State = WorkerNetbenchServerState;

    async fn connect(&self, addr: &SocketAddr) -> RussulaResult<TcpStream> {
        let listener = TcpListener::bind(addr).await.unwrap();
        println!("--- Worker listening on: {}", addr);

        let (stream, _local_addr) =
            listener
                .accept()
                .await
                .map_err(|err| RussulaError::Connect {
                    dbg: err.to_string(),
                })?;
        println!("Worker success connection: {addr}");

        Ok(stream)
    }

    async fn run_till_ready(&mut self, stream: &TcpStream) -> RussulaResult<()> {
        self.run_till_state(stream, WorkerNetbenchServerState::Ready)
            .await
    }

    async fn run_till_state(
        &mut self,
        stream: &TcpStream,
        state: Self::State,
    ) -> RussulaResult<()> {
        while !self.state.eq(state) {
            println!("curr worker state--------{:?}", self.state);
            self.state.run(stream).await;
        }

        Ok(())
    }

    fn state(&self) -> Self::State {
        self.state
    }
}

#[async_trait]
impl StateApi for WorkerNetbenchServerState {
    async fn run(&mut self, stream: &TcpStream) {
        match self {
            WorkerNetbenchServerState::WaitPeerInit => {
                let msg = network_utils::recv_msg(stream).await.unwrap();
                self.process_msg(msg);

                let state = self.as_bytes();
                network_utils::send_msg(stream, state.into()).await.unwrap();
            }
            WorkerNetbenchServerState::Ready => self.next(),
            WorkerNetbenchServerState::Run => self.next(),
            WorkerNetbenchServerState::Done => self.next(),
        }
    }

    fn eq(&self, other: Self) -> bool {
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
            WorkerNetbenchServerState::Done => {
                matches!(other, WorkerNetbenchServerState::Done)
            }
        }
    }

    fn transition_step(&self) -> TransitionStep {
        match self {
            WorkerNetbenchServerState::WaitPeerInit => {
                TransitionStep::PeerDriven(CoordNetbenchServerState::CheckPeer.as_bytes())
            }
            WorkerNetbenchServerState::Ready => {
                TransitionStep::PeerDriven(CoordNetbenchServerState::RunPeer.as_bytes())
            }
            WorkerNetbenchServerState::Run => {
                TransitionStep::PeerDriven(CoordNetbenchServerState::KillPeer.as_bytes())
            }
            WorkerNetbenchServerState::Done => TransitionStep::Finished,
        }
    }

    fn next(&mut self) {
        *self = match self {
            WorkerNetbenchServerState::WaitPeerInit => WorkerNetbenchServerState::Ready,
            WorkerNetbenchServerState::Ready => WorkerNetbenchServerState::Run,
            WorkerNetbenchServerState::Run => WorkerNetbenchServerState::Done,
            WorkerNetbenchServerState::Done => WorkerNetbenchServerState::Done,
        };
    }

    fn process_msg(&mut self, msg: Bytes) {
        if let TransitionStep::PeerDriven(peer_msg) = self.transition_step() {
            if peer_msg == msg {
                self.next();
            }
            println!(
                "worker {:?} {:?} {:?}",
                std::str::from_utf8(peer_msg),
                std::str::from_utf8(&msg),
                self
            );
        }
    }

    fn as_bytes(&self) -> &'static [u8] {
        match self {
            WorkerNetbenchServerState::WaitPeerInit => b"server_wait_coord_init",
            WorkerNetbenchServerState::Ready => b"server_ready",
            WorkerNetbenchServerState::Run => b"server_wait_peer_done",
            WorkerNetbenchServerState::Done => b"server_done",
        }
    }

    fn from_bytes(bytes: &[u8]) -> RussulaResult<Self> {
        let state = match bytes {
            b"server_wait_coord_init" => WorkerNetbenchServerState::WaitPeerInit,
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