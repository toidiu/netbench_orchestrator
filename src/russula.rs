// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::protocol::SockProtocol;
use std::{collections::BTreeSet, net::SocketAddr};

mod error;
mod netbench;
mod protocol;

use error::RussulaResult;
use protocol::Protocol;

use self::protocol::NextTransitionMsg;
use self::protocol::StateApi;

pub struct Russula<P: Protocol> {
    role: Vec<SockProtocol<P>>,
}

// TODO
// - handle coord retry on connect
//
// - curr_state ()
// - peer_state_update (to, from)
//   - ack msg (send curr state)
// - self_state_updated (to, from)
//
// - worker groups (server, client)
// D- move connect to protocol impl

impl<P: Protocol> Russula<P> {
    pub fn new(addr: BTreeSet<SocketAddr>, protocol: P) -> Self {
        let mut map = Vec::new();
        addr.into_iter().for_each(|addr| {
            map.push((addr, protocol.clone()));
        });
        Self { role: map }
    }

    pub async fn connect(&self) -> RussulaResult<()> {
        let mut v = Vec::new();
        for (addr, protocol) in self.role.iter() {
            let stream = protocol.connect(addr).await?;
            println!("Coordinator: successfully connected to {}", addr);
            v.push((stream, protocol));
        }

        for (stream, protocol) in v.into_iter() {
            // TODO start instead
            protocol.send_msg(stream, protocol.state()).await?;
        }

        Ok(())
    }

    pub async fn start(&self) {
        for (_addr, protocol) in self.role.iter() {
            protocol.start();
        }
    }

    pub async fn kill(&self) {
        for (_addr, protocol) in self.role.iter() {
            protocol.kill();
        }
    }

    #[allow(unused_variables)]
    pub async fn check_peer_state(&self, state: P::State) -> RussulaResult<bool> {
        let mut matches = true;
        for (_addr, protocol) in self.role.iter() {
            let protocol_state = protocol.peer_state();
            matches &= state.eq(protocol_state);
            println!("{:?} {:?} {}", protocol_state, state, matches);
        }
        Ok(matches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::russula::netbench::{
        NetbenchOrchProtocol, NetbenchOrchState, NetbenchWorkerProtocol,
    };
    use std::str::FromStr;

    #[tokio::test]
    async fn test() {
        let w1_sock = SocketAddr::from_str("127.0.0.1:8991").unwrap();
        let w2_sock = SocketAddr::from_str("127.0.0.1:8992").unwrap();

        let w1 = tokio::spawn(async move {
            let worker = Russula::new(
                BTreeSet::from_iter([w1_sock]),
                NetbenchWorkerProtocol::new(),
            );
            worker.connect().await.unwrap();
            worker
        });
        let w2 = tokio::spawn(async move {
            let worker = Russula::new(
                BTreeSet::from_iter([w2_sock]),
                NetbenchWorkerProtocol::new(),
            );
            worker.connect().await.unwrap();
            worker
        });

        let c1 = tokio::spawn(async move {
            let addr = BTreeSet::from_iter([w1_sock, w2_sock]);
            let coord = Russula::new(addr, NetbenchOrchProtocol::new());
            coord.connect().await.unwrap();
            // assert!(coord.state, Ready)
            // do something
            // assert!(coord.state, WaitPeerDone)
            // assert!(coord.state, Done)
            coord
        });

        let join = tokio::join!(w1, w2, c1);
        let coord = join.2.unwrap();
        coord
            .check_peer_state(NetbenchOrchState::WaitPeerDone)
            .await
            .unwrap();
        coord.kill().await;

        assert!(1 == 43)
    }
}
