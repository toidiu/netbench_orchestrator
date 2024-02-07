// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::{
    error::{RussulaError, RussulaResult},
    event::{EventRecorder, EventType},
    netbench::client::WorkerState,
    network_utils::Msg,
    protocol::{private, Protocol},
    StateApi, TransitionStep,
};
use async_trait::async_trait;
use core::fmt::Debug;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tracing::{debug, info};

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum CoordState {
    CheckWorker,
    Ready,
    RunWorker,
    WorkersRunning,
    Done,
}

#[derive(Debug, Clone)]
pub struct CoordProtocol {
    state: CoordState,
    worker_state: WorkerState,
    event_recorder: EventRecorder,
}

impl CoordProtocol {
    pub fn new() -> Self {
        CoordProtocol {
            state: CoordState::CheckWorker,
            worker_state: WorkerState::WaitCoordInit,
            event_recorder: EventRecorder::default(),
        }
    }
}

impl private::Protocol for CoordProtocol {
    fn event_recorder(&mut self) -> &mut EventRecorder {
        &mut self.event_recorder
    }
}

#[async_trait]
impl Protocol for CoordProtocol {
    type State = CoordState;
    fn name(&self) -> String {
        format!("client-c-{}", 0)
    }

    async fn connect(&self, addr: &SocketAddr) -> RussulaResult<TcpStream> {
        info!("--- Coordinator: attempt to connect on: {}", addr);

        let connect = TcpStream::connect(addr).await.map_err(RussulaError::from)?;
        Ok(connect)
    }

    fn update_peer_state(&mut self, msg: Msg) -> RussulaResult<()> {
        self.worker_state = WorkerState::from_msg(msg)?;
        debug!("{} ... peer_state {:?}", self.name(), self.worker_state);

        Ok(())
    }

    fn state(&self) -> &Self::State {
        &self.state
    }

    fn state_mut(&mut self) -> &mut Self::State {
        &mut self.state
    }

    fn ready_state(&self) -> Self::State {
        CoordState::Ready
    }

    fn done_state(&self) -> Self::State {
        CoordState::Done
    }

    fn worker_running_state(&self) -> Self::State {
        CoordState::WorkersRunning
    }

    async fn run(&mut self, stream: &TcpStream) -> RussulaResult<Option<Msg>> {
        match self.state_mut() {
            CoordState::CheckWorker => {
                self.state().notify_peer(stream).await?;
                self.await_next_msg(stream).await
            }
            CoordState::Ready => {
                let name = self.name();
                self.state_mut()
                    .transition_self_or_user_driven(stream, name)
                    .await?;
                Ok(None)
            }
            CoordState::RunWorker => {
                self.state().notify_peer(stream).await?;
                self.await_next_msg(stream).await
            }
            CoordState::WorkersRunning => {
                self.state().notify_peer(stream).await?;
                self.await_next_msg(stream).await
            }
            CoordState::Done => {
                self.state().notify_peer(stream).await?;
                Ok(None)
            }
        }
    }
}

#[async_trait]
impl StateApi for CoordState {
    fn name_prefix(&self) -> String {
        "client-coord".to_string()
    }

    fn transition_step(&self) -> TransitionStep {
        match self {
            CoordState::CheckWorker => TransitionStep::AwaitNext(WorkerState::Ready.as_bytes()),
            CoordState::Ready => TransitionStep::UserDriven,
            CoordState::RunWorker => TransitionStep::AwaitNext(WorkerState::Running(0).as_bytes()),
            CoordState::WorkersRunning => {
                TransitionStep::AwaitNext(WorkerState::Stopped.as_bytes())
            }
            CoordState::Done => TransitionStep::Finished,
        }
    }

    fn next_state(&self) -> Self {
        match self {
            CoordState::CheckWorker => CoordState::Ready,
            CoordState::Ready => CoordState::RunWorker,
            CoordState::RunWorker => CoordState::WorkersRunning,
            CoordState::WorkersRunning => CoordState::Done,
            CoordState::Done => CoordState::Done,
        }
    }
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn netbench_state() {}
}
