// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::network_utils::Msg;
use crate::russula::{network_utils, RussulaResult};
use async_trait::async_trait;
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

    fn name(&self) -> String;
    // TODO use version and app to negotiate version
    // fn version(&self) {1, 2}
    // fn app_name(&self) { "netbench" }

    async fn connect(&self, addr: &SocketAddr) -> RussulaResult<TcpStream>;
    async fn run_till_ready(&mut self, stream: &TcpStream) -> RussulaResult<()>;
    async fn run_till_state(
        &mut self,
        stream: &TcpStream,
        state: Self::State,
    ) -> RussulaResult<()> {
        while !self.state().eq(&state) {
            let prev = *self.state();
            self.state_mut().run(stream).await?;

            println!(
                "{} state--------{:?} -> {:?}",
                self.name(),
                prev,
                self.state()
            );
        }
        Ok(())
    }

    async fn poll_state(
        &mut self,
        stream: &TcpStream,
        state: Self::State,
    ) -> RussulaResult<Poll<()>> {
        if !self.state().eq(&state) {
            let prev = *self.state();
            self.state_mut().run(stream).await?;
            println!(
                "{} state--------{:?} -> {:?}",
                self.name(),
                prev,
                self.state()
            );
        }
        let poll = if self.state().eq(&state) {
            Poll::Ready(())
        } else {
            Poll::Pending
        };
        Ok(poll)
    }

    async fn poll_current(&mut self, stream: &TcpStream) -> RussulaResult<Poll<()>> {
        let state = *self.state();
        self.poll_state(stream, state).await
    }

    async fn poll_next(&mut self, stream: &TcpStream) -> RussulaResult<Poll<()>> {
        let state = self.state().next_state();
        self.poll_state(stream, state).await
    }

    fn state(&self) -> &Self::State;
    fn state_mut(&mut self) -> &mut Self::State;
}

pub type SockProtocol<P> = (SocketAddr, P);

#[derive(Debug)]
pub enum TransitionStep {
    Ready,
    SelfDriven,
    UserDriven,
    AwaitPeer(&'static [u8]),
    Finished,
}

#[async_trait]
pub trait StateApi: Sized + Send + Sync + Debug {
    fn name(&self) -> String;
    async fn run(&mut self, stream: &TcpStream) -> RussulaResult<()>;

    fn eq(&self, other: &Self) -> bool;
    fn transition_step(&self) -> TransitionStep;
    fn next_state(&self) -> Self;

    fn as_bytes(&self) -> &'static [u8];
    fn from_bytes(bytes: &[u8]) -> RussulaResult<Self>;
    async fn notify_peer(&self, stream: &TcpStream) -> RussulaResult<()> {
        let msg = Msg::new(self.as_bytes().into());
        println!("{} ----> send msg {:?}", self.name(), msg);
        network_utils::send_msg(stream, msg).await
    }

    async fn transition_next(&mut self, stream: &TcpStream) {
        println!(
            "{}------------- moving to next state {:?}",
            self.name(),
            self
        );

        *self = self.next_state();
        self.notify_peer(stream).await.unwrap();
    }
    async fn await_peer_msg(&mut self, stream: &TcpStream) -> RussulaResult<()> {
        let msg = network_utils::recv_msg(stream).await?;
        println!("{} <----recv msg {}", self.name(), msg);
        self.process_msg(stream, msg).await
    }
    async fn process_msg(&mut self, stream: &TcpStream, recv_msg: Msg) -> RussulaResult<()> {
        if let TransitionStep::AwaitPeer(expected_msg) = self.transition_step() {
            if expected_msg == recv_msg.as_bytes() {
                self.transition_next(stream).await;
            } else {
                self.notify_peer(stream).await?
            }
            println!(
                "{} ========transition: {}, expect_msg: {:?} recv_msg: {:?}",
                self.name(),
                expected_msg == recv_msg.as_bytes(),
                std::str::from_utf8(expected_msg),
                recv_msg,
            );
            // todo remove
            self.notify_peer(stream).await?
        }

        Ok(())
    }
}
