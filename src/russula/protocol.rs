// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{
    error::RussulaError,
    event::EventType,
    network_utils,
    network_utils::Msg,
    states::{StateApi, TransitionStep},
    RussulaResult,
};
use async_trait::async_trait;
use bytes::Bytes;
use core::{fmt::Debug, task::Poll, time::Duration};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tracing::{debug, info};

const NOTIFY_DONE_TIMEOUT: Duration = Duration::from_secs(1);

pub type SockProtocol<P> = (SocketAddr, P);

pub(crate) struct ProtocolInstance<P: Protocol> {
    pub addr: SocketAddr,
    pub stream: TcpStream,
    pub protocol: P,
}

#[async_trait]
pub trait Protocol: private::Protocol + Clone {
    type State: StateApi + Debug + Copy;

    // TODO use version and app to negotiate version
    fn name(&self) -> String;
    async fn connect(&self, addr: &SocketAddr) -> RussulaResult<TcpStream>;
    async fn run(&mut self, stream: &TcpStream) -> RussulaResult<Option<Msg>>;
    fn update_peer_state(&mut self, msg: Msg) -> RussulaResult<()>;
    fn state(&self) -> &Self::State;
    fn state_mut(&mut self) -> &mut Self::State;
    fn ready_state(&self) -> Self::State;
    fn done_state(&self) -> Self::State;

    async fn poll_ready(&mut self, stream: &TcpStream) -> RussulaResult<Poll<()>> {
        let ready_state = self.ready_state();
        self.poll_state(stream, &ready_state).await
    }

    // If the peer is not at the desired state then attempt to make progress by invoking the
    // 'run_current' action
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
        if let Some(msg) = self.run(stream).await? {
            self.update_peer_state(msg)?;
        }
        // FIXME
        // self.event(event)
        Ok(())
    }

    fn is_done_state(&self) -> bool {
        matches!(self.state().transition_step(), TransitionStep::Finished)
    }

    async fn await_next_msg(&mut self, stream: &TcpStream) -> RussulaResult<Option<Msg>> {
        if !matches!(self.state().transition_step(), TransitionStep::AwaitNext(_)) {
            panic!(
                "expected AwaitNext but found: {:?}",
                self.state().transition_step()
            );
        }
        // loop until we receive a transition msg from peer or drain all msg from queue.
        // recv_msg aborts if the read queue is empty
        let mut last_msg = None;
        // Continue to read from stream until:
        // - the msg results in a transition
        // - there is no more data available (drained all messages)
        // - there is a error while reading
        loop {
            match network_utils::recv_msg(stream).await {
                Ok(msg) => {
                    debug!(
                        "{} <---- recv msg {}",
                        self.name(),
                        std::str::from_utf8(&msg.data).unwrap()
                    );

                    let state = self.state();
                    let should_transition = state.matches_transition_msg(stream, &msg).await?;
                    last_msg = Some(msg);
                    if should_transition {
                        self.state_mut().transition_next(stream).await?;
                        break;
                    }
                }
                Err(RussulaError::NetworkBlocked { dbg }) => {
                    // TODO this might not be necessary but is nice way to confirm the
                    // system makes progress. Test and figure out if its possible to
                    // remome this.
                    //
                    // notify the peer to that we continue to make progress
                    self.state().notify_peer(stream).await?;
                    break;
                }
                Err(err) => return Err(err),
            }
        }

        Ok(last_msg)
    }

    fn event(&mut self, event: EventType);
}

pub(crate) mod private {
    pub trait Protocol {}
}
