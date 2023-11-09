// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::network_utils::Msg;
use crate::russula::{network_utils, RussulaResult};
use async_trait::async_trait;
use bytes::Bytes;
use core::{fmt::Debug, task::Poll};
use serde::Serialize;
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
    async fn poll_ready(&mut self, stream: &TcpStream) -> RussulaResult<Poll<()>>;

    async fn poll_state(
        &mut self,
        stream: &TcpStream,
        state: Self::State,
    ) -> RussulaResult<Poll<()>> {
        if !self.state().eq(&state) {
            let prev = *self.state();
            let name = self.name();
            self.state_mut().run(stream, name).await?;
            println!(
                "{} poll_state--------{:?} -> {:?}",
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
    AwaitPeer(Bytes),
    Finished,
}

#[async_trait]
pub trait StateApi: Sized + Send + Sync + Debug + Serialize {
    fn name(&self) -> String;
    async fn run(&mut self, stream: &TcpStream, name: String) -> RussulaResult<()>;
    fn transition_step(&self) -> TransitionStep;
    fn next_state(&self) -> Self;

    async fn notify_peer(&self, stream: &TcpStream) -> RussulaResult<usize> {
        let msg = Msg::new(self.as_bytes());
        println!("{} ----> send msg {:?}", self.name(), msg);
        network_utils::send_msg(stream, msg).await
    }

    async fn transition_next(&mut self, stream: &TcpStream) -> RussulaResult<usize> {
        println!(
            "{}------------- moving to next state current: {:?}, next: {:?}",
            self.name(),
            self,
            self.next_state()
        );

        *self = self.next_state();
        self.notify_peer(stream).await
    }

    async fn await_peer_msg(&mut self, stream: &TcpStream) -> RussulaResult<()> {
        let mut did_transition = false;
        // drain all messages in the queue until we can transition to the next state
        while !did_transition {
            let mut msg = network_utils::recv_msg(stream).await?;
            println!("{} <---- recv msg {:?}", self.name(), msg);
            did_transition = self.process_msg_and_transition(stream, &mut msg).await?;
        }

        Ok(())
    }

    async fn process_msg_and_transition(
        &mut self,
        stream: &TcpStream,
        recv_msg: &mut Msg,
    ) -> RussulaResult<bool> {
        if let TransitionStep::AwaitPeer(expected_msg) = self.transition_step() {
            let did_transition = if expected_msg == recv_msg.as_bytes() {
                self.transition_next(stream).await?;
                true
            } else {
                self.notify_peer(stream).await?;
                false
            };
            println!(
                "{} ========transition: {}, expect_msg: {:?} recv_msg: {:?}",
                self.name(),
                expected_msg == recv_msg.as_bytes(),
                std::str::from_utf8(&expected_msg),
                recv_msg,
            );
            Ok(did_transition)
        } else {
            Ok(false)
        }
    }

    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }

    fn as_bytes(&self) -> Bytes {
        serde_json::to_string(self).unwrap().into()
    }
}
