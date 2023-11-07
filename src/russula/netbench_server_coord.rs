// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::{
    error::{RussulaError, RussulaResult},
    netbench_server_worker::WorkerNetbenchServerState,
    protocol::Protocol,
    StateApi, TransitionStep,
};
use async_trait::async_trait;
use core::fmt::Debug;
use std::net::SocketAddr;
use tokio::net::TcpStream;

#[derive(Copy, Clone, Debug)]
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
        "coord".to_string()
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

    async fn run_till_ready(&mut self, stream: &TcpStream) -> RussulaResult<()> {
        self.run_till_state(stream, CoordNetbenchServerState::Ready)
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
    async fn run(&mut self, stream: &TcpStream) -> RussulaResult<()> {
        match self {
            CoordNetbenchServerState::CheckPeer => {
                self.notify_peer(stream).await?;
                self.await_peer_msg(stream).await?;
            }
            CoordNetbenchServerState::Ready => {
                self.await_peer_msg(stream).await?;
                self.transition_next(stream).await;
            }
            CoordNetbenchServerState::RunPeer => {
                self.await_peer_msg(stream).await?;
                self.transition_next(stream).await;
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
                TransitionStep::AwaitPeer(WorkerNetbenchServerState::Done.as_bytes())
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

    fn as_bytes(&self) -> &'static [u8] {
        match self {
            CoordNetbenchServerState::CheckPeer => b"coord_check_peer",
            CoordNetbenchServerState::Ready => b"coord_ready",
            CoordNetbenchServerState::RunPeer => b"coord_run_peer",
            CoordNetbenchServerState::KillPeer => b"coord_wait_peer_done",
            CoordNetbenchServerState::Done => b"coord_done",
        }
    }

    fn from_bytes(bytes: &[u8]) -> RussulaResult<Self> {
        let state = match bytes {
            b"coord_ready" => CoordNetbenchServerState::Ready,
            b"coord_run_peer" => CoordNetbenchServerState::RunPeer,
            b"coord_wait_peer_done" => CoordNetbenchServerState::KillPeer,
            b"coord_done" => CoordNetbenchServerState::Done,
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
