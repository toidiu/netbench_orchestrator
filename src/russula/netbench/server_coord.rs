// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::{
    error::{RussulaError, RussulaResult},
    netbench::server_worker::WorkerState,
    protocol::Protocol,
    StateApi, TransitionStep,
};
use async_trait::async_trait;
use core::{fmt::Debug, task::Poll};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::TcpStream;

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum CoordState {
    CheckPeer,
    Ready,
    RunPeer,
    KillPeer,
    Done,
}

#[derive(Clone, Copy)]
pub struct CoordProtocol {
    state: CoordState,
}

impl CoordProtocol {
    pub fn new() -> Self {
        CoordProtocol {
            state: CoordState::CheckPeer,
        }
    }
}

#[async_trait]
impl Protocol for CoordProtocol {
    type State = CoordState;
    fn name(&self) -> String {
        format!("[coord-{}]", 0)
    }

    async fn connect(&self, addr: &SocketAddr) -> RussulaResult<TcpStream> {
        println!("--- Coordinator: attempt to connect on: {}", addr);

        let connect = TcpStream::connect(addr).await.map_err(RussulaError::from)?;
        Ok(connect)
    }

    async fn poll_ready(&mut self, stream: &TcpStream) -> RussulaResult<Poll<()>> {
        self.poll_state(stream, CoordState::Ready).await
    }

    fn state(&self) -> &Self::State {
        &self.state
    }

    fn state_mut(&mut self) -> &mut Self::State {
        &mut self.state
    }
}

#[async_trait]
impl StateApi for CoordState {
    fn name(&self) -> String {
        format!("[coord-{}]", 0)
    }

    async fn run(&mut self, stream: &TcpStream, _name: String) -> RussulaResult<()> {
        match self {
            CoordState::CheckPeer => {
                self.notify_peer(stream).await?;
                self.await_peer_msg(stream).await?;
            }
            CoordState::Ready => {
                // self.await_peer_msg(stream).await?;
                self.transition_next(stream).await?;
            }
            CoordState::RunPeer => {
                // self.await_peer_msg(stream).await?;
                self.transition_next(stream).await?;
            }
            CoordState::KillPeer => {
                self.notify_peer(stream).await?;
                self.await_peer_msg(stream).await?;
            }
            CoordState::Done => {
                self.notify_peer(stream).await?;
            }
        }
        Ok(())
    }

    fn transition_step(&self) -> TransitionStep {
        match self {
            CoordState::CheckPeer => TransitionStep::AwaitPeer(WorkerState::Ready.as_bytes()),
            CoordState::Ready => TransitionStep::UserDriven,
            CoordState::RunPeer => TransitionStep::UserDriven,
            CoordState::KillPeer => {
                TransitionStep::AwaitPeer(WorkerState::RunningAwaitPeer(0).as_bytes())
            }
            CoordState::Done => TransitionStep::Finished,
        }
    }

    fn next_state(&self) -> Self {
        match self {
            CoordState::CheckPeer => CoordState::Ready,
            CoordState::Ready => CoordState::RunPeer,
            CoordState::RunPeer => CoordState::KillPeer,
            CoordState::KillPeer => CoordState::Done,
            CoordState::Done => CoordState::Done,
        }
    }
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn netbench_state() {}
}