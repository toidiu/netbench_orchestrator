// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{error::RussulaError, network_utils::Msg};
use crate::russula::{network_utils, RussulaResult};
use async_trait::async_trait;
use bytes::Bytes;
use core::{fmt::Debug, task::Poll, time::Duration};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tracing::{debug, info};

const NOTIFY_DONE_TIMEOUT: Duration = Duration::from_secs(1);

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
        let ready_state = self.ready_state();
        self.poll_state(stream, &ready_state).await
    }

    async fn poll_state(
        &mut self,
        stream: &TcpStream,
        state: &Self::State,
    ) -> RussulaResult<Poll<()>> {
        if !self.state().eq(state) {
            let prev = *self.state();
            self.run_current(stream).await?;
            debug!(
                "{} poll_state--------{:?} -> {:?}",
                self.name(),
                prev,
                self.state()
            );
        }
        // Notify the peer that the protocol has reached a terminal state
        if self.is_done_state() {
            // Notify 3 time in case of packet loss.. this is best effort
            for _i in 0..3 {
                match self.run_current(stream).await {
                    Ok(_) => (),
                    // We notify the peer of the Done state multiple times. Since the peer could
                    // have killed the connection in the meantime, its better to ignore network
                    // failures
                    Err(RussulaError::NetworkConnectionRefused { dbg })
                    | Err(RussulaError::NetworkBlocked { dbg })
                    | Err(RussulaError::NetworkFail { dbg }) => {
                        debug!("Ignore network failure since coordination is Done.")
                    }
                    Err(err) => return Err(err),
                }
                tokio::time::sleep(NOTIFY_DONE_TIMEOUT).await;
            }
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
        if let Some(msg) = self.run(stream).await? {
            self.update_peer_state(msg)?;
        }
        Ok(())
    }

    fn update_peer_state(&mut self, msg: Msg) -> RussulaResult<()>;
    fn state(&self) -> &Self::State;
    fn state_mut(&mut self) -> &mut Self::State;
    fn ready_state(&self) -> Self::State;
    fn done_state(&self) -> Self::State;
    fn is_done_state(&self) -> bool {
        matches!(self.state().transition_step(), TransitionStep::Finished)
    }

    async fn run(&mut self, stream: &TcpStream) -> RussulaResult<Option<Msg>>;

    async fn await_next_msg(&mut self, stream: &TcpStream) -> RussulaResult<Msg> {
        if !matches!(self.state().transition_step(), TransitionStep::AwaitNext(_)) {
            panic!(
                "expected AwaitNext but found: {:?}",
                self.state().transition_step()
            );
        }
        // loop until we receive a transition msg from peer or drain all msg from queue.
        // recv_msg aborts if the read queue is empty
        let mut last_msg;
        loop {
            last_msg = network_utils::recv_msg(stream).await?;
            debug!("{} <---- recv msg {:?}", self.name(), last_msg);

            let state = self.state();
            if state.matches_transition_msg(stream, &mut last_msg).await? {
                self.state_mut().transition_next(stream).await?;
                break;
            } else {
                let fut = self.state().notify_peer(stream);
                fut.await?;
            }
        }

        Ok(last_msg)
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

    fn transition_step(&self) -> TransitionStep;
    fn next_state(&self) -> Self;

    async fn notify_peer(&self, stream: &TcpStream) -> RussulaResult<usize> {
        let msg = Msg::new(self.as_bytes());
        debug!("{} ----> send msg {:?}", self.name(stream), msg);
        network_utils::send_msg(stream, msg).await
    }

    async fn transition_self_or_user_driven(&mut self, stream: &TcpStream) -> RussulaResult<()> {
        info!(
            "{}------------- moving to next state current: {:?}, next: {:?}",
            self.name(stream),
            self,
            self.next_state()
        );

        *self = self.next_state();
        self.notify_peer(stream).await.map(|_| ())
    }

    async fn transition_next(&mut self, stream: &TcpStream) -> RussulaResult<()> {
        info!(
            "{}------------- moving to next state current: {:?}, next: {:?}",
            self.name(stream),
            self,
            self.next_state()
        );

        *self = self.next_state();
        self.notify_peer(stream).await.map(|_| ())
    }

    async fn matches_transition_msg(
        &self,
        stream: &TcpStream,
        recv_msg: &mut Msg,
    ) -> RussulaResult<bool> {
        if let TransitionStep::AwaitNext(expected_msg) = self.transition_step() {
            let should_transition_to_next = expected_msg == recv_msg.as_bytes();
            if should_transition_to_next {
                info!(
                    "{} transition: {}, expect_msg: {:?} recv_msg: {:?}",
                    self.name(stream),
                    should_transition_to_next,
                    std::str::from_utf8(&expected_msg),
                    recv_msg,
                );
            } else {
                debug!(
                    "{} transition: {}, expect_msg: {:?} recv_msg: {:?}",
                    self.name(stream),
                    should_transition_to_next,
                    std::str::from_utf8(&expected_msg),
                    recv_msg,
                );
            }
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

    fn from_msg(msg: Msg) -> RussulaResult<Self> {
        let msg_str = std::str::from_utf8(&msg.data).map_err(|_err| RussulaError::BadMsg {
            dbg: format!(
                "received a malformed msg. len: {} data: {:?}",
                msg.len, msg.data
            ),
        })?;

        serde_json::from_str(msg_str).map_err(|_err| RussulaError::BadMsg {
            dbg: format!(
                "received a malformed msg. len: {} data: {:?}",
                msg.len, msg.data
            ),
        })
    }
}
