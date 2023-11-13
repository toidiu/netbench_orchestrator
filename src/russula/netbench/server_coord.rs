// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::{
    error::{RussulaError, RussulaResult},
    netbench::server_worker::WorkerState,
    protocol::Protocol,
    StateApi, TransitionStep,
};
use async_trait::async_trait;
use core::fmt::Debug;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::TcpStream;

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum CoordState {
    CheckWorker,
    Ready,
    RunWorker,
    WorkersRunning,
    KillWorker,
    Done,
}

#[derive(Clone, Copy)]
pub struct CoordProtocol {
    state: CoordState,
}

impl CoordProtocol {
    pub fn new() -> Self {
        CoordProtocol {
            state: CoordState::CheckWorker,
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

    fn state(&self) -> &Self::State {
        &self.state
    }

    fn state_mut(&mut self) -> &mut Self::State {
        &mut self.state
    }

    fn state_ready(&self) -> Self::State {
        CoordState::Ready
    }
}

#[async_trait]
impl StateApi for CoordState {
    fn name(&self) -> String {
        format!("[coord-{}]", 0)
    }

    async fn run(&mut self, stream: &TcpStream, _name: String) -> RussulaResult<()> {
        match self {
            CoordState::CheckWorker => {
                self.notify_peer(stream).await?;
                self.await_next_msg(stream).await?;
            }
            CoordState::Ready => {
                self.transition_self_or_user_driven(stream).await?;
            }
            CoordState::RunWorker => {
                self.notify_peer(stream).await?;
                self.await_next_msg(stream).await?;
            }
            CoordState::WorkersRunning => {
                self.transition_self_or_user_driven(stream).await?;
            }
            CoordState::KillWorker => {
                self.notify_peer(stream).await?;
                self.await_next_msg(stream).await?;
            }
            CoordState::Done => {
                self.notify_peer(stream).await?;
            }
        }
        Ok(())
    }

    fn transition_step(&self) -> TransitionStep {
        match self {
            CoordState::CheckWorker => TransitionStep::AwaitNext(WorkerState::Ready.as_bytes()),
            CoordState::Ready => TransitionStep::UserDriven,
            CoordState::RunWorker => {
                TransitionStep::AwaitNext(WorkerState::RunningAwaitKill(0).as_bytes())
            }
            CoordState::WorkersRunning => TransitionStep::UserDriven,
            CoordState::KillWorker => TransitionStep::AwaitNext(WorkerState::Stopped.as_bytes()),
            CoordState::Done => TransitionStep::Finished,
        }
    }

    fn next_state(&self) -> Self {
        match self {
            CoordState::CheckWorker => CoordState::Ready,
            CoordState::Ready => CoordState::RunWorker,
            CoordState::RunWorker => CoordState::WorkersRunning,
            CoordState::WorkersRunning => CoordState::KillWorker,
            CoordState::KillWorker => CoordState::Done,
            CoordState::Done => CoordState::Done,
        }
    }
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn netbench_state() {}
}
