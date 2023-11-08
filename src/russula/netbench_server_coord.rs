// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::{
    error::{RussulaError, RussulaResult},
    netbench_server_worker::WorkerNetbenchServerState,
    protocol::Protocol,
    StateApi, TransitionStep,
};
use async_trait::async_trait;
use bytes::Bytes;
use core::{fmt::Debug, task::Poll};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::TcpStream;

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum CoordNetbenchServerState {
    CheckPeer,
    Ready,
    RunPeer,
    KillPeer,
    Done,
}

#[derive(Clone, Copy)]
pub struct NetbenchCoordServerProtocol {
    state: CoordNetbenchServerState,
}

impl NetbenchCoordServerProtocol {
    pub fn new() -> Self {
        NetbenchCoordServerProtocol {
            state: CoordNetbenchServerState::CheckPeer,
        }
    }
}

#[async_trait]
impl Protocol for NetbenchCoordServerProtocol {
    type State = CoordNetbenchServerState;
    fn name(&self) -> String {
        format!("[coord-{}]", 0)
    }

    async fn connect(&self, addr: &SocketAddr) -> RussulaResult<TcpStream> {
        println!("--- Coordinator: attempt to connect on: {}", addr);

        let connect = TcpStream::connect(addr)
            .await
            .map_err(|err| RussulaError::NetworkFail {
                dbg: err.to_string(),
            })?;

        Ok(connect)
    }

    async fn poll_ready(&mut self, stream: &TcpStream) -> RussulaResult<Poll<()>> {
        self.poll_state(stream, CoordNetbenchServerState::Ready)
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
impl StateApi for CoordNetbenchServerState {
    fn name(&self) -> String {
        format!("[coord-{}]", 0)
    }

    async fn run(&mut self, stream: &TcpStream, _name: String) -> RussulaResult<()> {
        match self {
            CoordNetbenchServerState::CheckPeer => {
                self.notify_peer(stream).await?;
                self.await_peer_msg(stream).await?;
            }
            CoordNetbenchServerState::Ready => {
                // self.await_peer_msg(stream).await?;
                self.transition_next(stream).await?;
            }
            CoordNetbenchServerState::RunPeer => {
                // self.await_peer_msg(stream).await?;
                self.transition_next(stream).await?;
            }
            CoordNetbenchServerState::KillPeer => {
                self.notify_peer(stream).await?;
                self.await_peer_msg(stream).await?;
            }
            CoordNetbenchServerState::Done => {
                self.notify_peer(stream).await?;
            }
        }
        Ok(())
    }

    fn transition_step(&self) -> TransitionStep {
        match self {
            CoordNetbenchServerState::CheckPeer => {
                TransitionStep::AwaitPeer(WorkerNetbenchServerState::Ready.as_bytes())
            }
            CoordNetbenchServerState::Ready => TransitionStep::UserDriven,
            CoordNetbenchServerState::RunPeer => TransitionStep::UserDriven,
            CoordNetbenchServerState::KillPeer => {
                TransitionStep::AwaitPeer(WorkerNetbenchServerState::RunningAwaitPeer(0).as_bytes())
            }
            CoordNetbenchServerState::Done => TransitionStep::Finished,
        }
    }

    fn next_state(&self) -> Self {
        match self {
            CoordNetbenchServerState::CheckPeer => CoordNetbenchServerState::Ready,
            CoordNetbenchServerState::Ready => CoordNetbenchServerState::RunPeer,
            CoordNetbenchServerState::RunPeer => CoordNetbenchServerState::KillPeer,
            CoordNetbenchServerState::KillPeer => CoordNetbenchServerState::Done,
            CoordNetbenchServerState::Done => CoordNetbenchServerState::Done,
        }
    }

    fn eq(&self, other: &Self) -> bool {
        match self {
            CoordNetbenchServerState::CheckPeer => {
                matches!(other, CoordNetbenchServerState::CheckPeer)
            }
            CoordNetbenchServerState::Ready => {
                matches!(other, CoordNetbenchServerState::Ready)
            }
            CoordNetbenchServerState::RunPeer => {
                matches!(other, CoordNetbenchServerState::RunPeer)
            }
            CoordNetbenchServerState::KillPeer => {
                matches!(other, CoordNetbenchServerState::KillPeer)
            }
            CoordNetbenchServerState::Done => {
                matches!(other, CoordNetbenchServerState::Done)
            }
        }
    }

    fn as_bytes(&self) -> Bytes {
        serde_json::to_string(self).unwrap().into()
    }

    fn from_bytes(bytes: &[u8]) -> RussulaResult<Self> {
        let state = serde_json::from_slice(bytes).unwrap();
        Ok(state)
    }
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn netbench_state() {}
}
