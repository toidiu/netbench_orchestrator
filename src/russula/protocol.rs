// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::network_utils::Msg;
use crate::russula::{network_utils, RussulaResult};
use async_trait::async_trait;
use bytes::Bytes;
use core::{fmt::Debug, task::Poll};
use serde::{Deserialize, Serialize};
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

    async fn poll_ready(&mut self, stream: &TcpStream) -> RussulaResult<Poll<()>> {
        let ready_state = self.state_ready();
        self.poll_state(stream, &ready_state).await
    }

    async fn poll_state(
        &mut self,
        stream: &TcpStream,
        state: &Self::State,
    ) -> RussulaResult<Poll<()>> {
        if !self.state().eq(state) {
            let prev = *self.state();
            let name = self.name();
            if let Some(msg) = self.state_mut().run(stream, name).await? {
                self.update_peer_state(msg)
            }
            println!(
                "{} poll_state--------{:?} -> {:?}",
                self.name(),
                prev,
                self.state()
            );
        }
        let poll = if self.state().eq(state) {
            Poll::Ready(())
        } else {
            Poll::Pending
        };
        Ok(poll)
    }

    async fn run_current(&mut self, stream: &TcpStream) -> RussulaResult<()> {
        let name = self.name();
        if let Some(msg) = self.state_mut().run(stream, name).await? {
            self.update_peer_state(msg)
        }
        Ok(())
    }

    fn update_peer_state(&mut self, msg: Msg) {}
    fn state(&self) -> &Self::State;
    fn state_mut(&mut self) -> &mut Self::State;
    fn state_ready(&self) -> Self::State;
    fn is_done_state(&self) -> bool {
        matches!(self.state().transition_step(), TransitionStep::Finished)
    }
}

pub type SockProtocol<P> = (SocketAddr, P);

#[derive(Debug)]
pub enum TransitionStep {
    // State machine is responsible for moving to the next state
    SelfDriven,
    // Wait for user input before moving to the next state
    UserDriven,
    // Wait for a peer msg before moving to the next state
    AwaitNext(Bytes),
    // Final step in the state machine with no next transitions
    Finished,
}

#[async_trait]
pub trait StateApi: Sized + Send + Sync + Debug + Serialize + for<'a> Deserialize<'a> {
    fn name_prefix(&self) -> String;

    fn name(&self, stream: &TcpStream) -> String {
        format!(
            "[{}-{}]",
            self.name_prefix(),
            stream.local_addr().unwrap().port()
        )
    }

    async fn run(&mut self, stream: &TcpStream, _name: String) -> RussulaResult<Option<Msg>>;
    fn transition_step(&self) -> TransitionStep;
    fn next_state(&self) -> Self;

    async fn notify_peer(&self, stream: &TcpStream) -> RussulaResult<usize> {
        let msg = Msg::new(self.as_bytes());
        println!("{} ----> send msg {:?}", self.name(stream), msg);
        network_utils::send_msg(stream, msg).await
    }

    async fn transition_self_or_user_driven(&mut self, stream: &TcpStream) -> RussulaResult<()> {
        println!(
            "{}------------- moving to next state current: {:?}, next: {:?}",
            self.name(stream),
            self,
            self.next_state()
        );

        *self = self.next_state();
        self.notify_peer(stream).await.map(|_| ())
    }

    async fn transition_next(&mut self, stream: &TcpStream) -> RussulaResult<()> {
        println!(
            "{}------------- moving to next state current: {:?}, next: {:?}",
            self.name(stream),
            self,
            self.next_state()
        );

        *self = self.next_state();
        self.notify_peer(stream).await.map(|_| ())
    }

    async fn await_next_msg(&mut self, stream: &TcpStream) -> RussulaResult<Msg> {
        if !matches!(self.transition_step(), TransitionStep::AwaitNext(_)) {
            panic!("expected AwaitNext but found: {:?}", self.transition_step());
        }
        // loop until we receive a transition msg from peer or drain all msg from queue.
        // recv_msg aborts if the read queue is empty
        let mut last_msg;
        loop {
            last_msg = network_utils::recv_msg(stream).await?;
            println!("{} <---- recv msg {:?}", self.name(stream), last_msg);

            if self.matches_transition_msg(stream, &mut last_msg).await? {
                self.transition_next(stream).await?;
                break;
            } else {
                self.notify_peer(stream).await?;
            }
        }

        Ok(last_msg)
    }

    async fn matches_transition_msg(
        &mut self,
        stream: &TcpStream,
        recv_msg: &mut Msg,
    ) -> RussulaResult<bool> {
        if let TransitionStep::AwaitNext(expected_msg) = self.transition_step() {
            let should_transition_to_next = expected_msg == recv_msg.as_bytes();
            println!(
                "{} ========transition: {}, expect_msg: {:?} recv_msg: {:?}",
                self.name(stream),
                expected_msg == recv_msg.as_bytes(),
                std::str::from_utf8(&expected_msg),
                recv_msg,
            );
            Ok(should_transition_to_next)
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

    fn from_msg(msg: Msg) -> Self {
        serde_json::from_str(std::str::from_utf8(&msg.data).unwrap()).unwrap()
    }
}
