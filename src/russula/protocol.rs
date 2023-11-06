// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::{network_utils, RussulaResult};
use async_trait::async_trait;
use bytes::Bytes;
use core::{fmt::Debug, task::Poll};
use std::net::SocketAddr;
use tokio::net::TcpStream;

pub(crate) struct RussulaPeer<P: Protocol> {
    pub addr: SocketAddr,
    pub stream: TcpStream,
    pub protocol: P,
}

#[async_trait]
pub trait Protocol: Clone {
    type State: StateApi + Debug + Copy;

    // TODO use version and app to negotiate version
    // fn version(&self) {1, 2}
    // fn app_name(&self) { "netbench" }

    async fn connect(&self, addr: &SocketAddr) -> RussulaResult<TcpStream>;
    async fn run_till_ready(&mut self, stream: &TcpStream) -> RussulaResult<()>;
    async fn run_till_state(&mut self, stream: &TcpStream, state: Self::State)
        -> RussulaResult<()>;
    async fn poll_state(
        &mut self,
        stream: &TcpStream,
        state: Self::State,
    ) -> RussulaResult<Poll<()>>;

    async fn poll_current(&mut self, stream: &TcpStream) -> RussulaResult<Poll<()>> {
        let state = *self.state();
        self.poll_state(stream, state).await
    }

    async fn poll_next(&mut self, stream: &TcpStream) -> RussulaResult<Poll<()>> {
        let state = self.state().next_state();

        self.poll_state(stream, state).await
    }

    fn state(&self) -> &Self::State;
}

pub type SockProtocol<P> = (SocketAddr, P);

#[derive(Debug)]
pub enum TransitionStep {
    Ready,
    UserDriven,
    AwaitPeerState(&'static [u8]),
    Finished,
}

#[async_trait]
pub trait StateApi: Sized + Send + Sync + Debug {
    async fn run(&mut self, stream: &TcpStream) -> RussulaResult<()>;
    fn eq(&self, other: &Self) -> bool;
    fn transition_step(&self) -> TransitionStep;
    fn transition_next(&mut self);
    fn next_state(&self) -> Self;

    fn as_bytes(&self) -> &'static [u8];
    fn from_bytes(bytes: &[u8]) -> RussulaResult<Self>;
    async fn notify_peer(&self, stream: &TcpStream) -> RussulaResult<()> {
        network_utils::send_msg(stream, self.as_bytes().into()).await
    }

    async fn await_peer_msg(&mut self, stream: &TcpStream) -> RussulaResult<()> {
        let msg = network_utils::recv_msg(stream).await?;
        self.process_msg(stream, msg).await
    }
    async fn process_msg(&mut self, stream: &TcpStream, recv_msg: Bytes) -> RussulaResult<()> {
        if let TransitionStep::AwaitPeerState(transition_msg) = self.transition_step() {
            if transition_msg == recv_msg {
                self.transition_next();
            }
            println!(
                "========transition_msg: {:?} recv_msg: {:?} state: {:?}",
                std::str::from_utf8(transition_msg),
                std::str::from_utf8(&recv_msg),
                self
            );
            self.notify_peer(stream).await?
        }

        Ok(())
    }
}
