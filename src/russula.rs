// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::protocol::SockProtocol;
use std::{collections::BTreeSet, net::SocketAddr};
use tokio::net::TcpStream;

mod error;
mod netbench;
mod protocol;

use error::RussulaResult;
use protocol::Protocol;

use self::protocol::NextTransitionMsg;
use self::protocol::StateApi;

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

struct RussulaPeer<P: Protocol> {
    addr: SocketAddr,
    stream: TcpStream,
    protocol: P,
}

pub struct Russula<P: Protocol> {
    peer_list: Vec<RussulaPeer<P>>,
}

impl<P: Protocol> Russula<P> {
    pub async fn start(&self) {
        for peer in self.peer_list.iter() {
            peer.protocol.start(&self.peer_list).unwrap();
        }
    }

    #[allow(unused_variables)]
    pub async fn check_peer_state(&self, state: P::State) -> RussulaResult<bool> {
        let mut matches = true;
        for peer in self.peer_list.iter() {
            let protocol_state = peer.protocol.peer_state();
            matches &= state.eq(protocol_state);
            println!("{:?} {:?} {}", protocol_state, state, matches);
        }
        Ok(matches)
    }
}

pub struct RussulaBuilder<P: Protocol> {
    peer_list: Vec<SockProtocol<P>>,
}

impl<P: Protocol> RussulaBuilder<P> {
    pub fn new(addr: BTreeSet<SocketAddr>, _protocol: P) -> Self {
        let mut map = Vec::new();
        addr.into_iter().for_each(|addr| {
            map.push((addr, P::default()));
        });
        Self { peer_list: map }
    }

    pub async fn build(self) -> RussulaResult<Russula<P>> {
        let mut stream_protocol_list = Vec::new();
        for (addr, protocol) in self.peer_list.into_iter() {
            let stream = protocol.connect(&addr).await?;
            println!("Coordinator: successfully connected to {}", addr);
            stream_protocol_list.push(RussulaPeer {
                addr,
                stream,
                protocol,
            });
        }

        Ok(Russula {
            peer_list: stream_protocol_list,
        })
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
            let worker = RussulaBuilder::new(
                BTreeSet::from_iter([w1_sock]),
                NetbenchWorkerProtocol::new(),
            );
            let worker = worker.build().await.unwrap();
            worker.start().await;
            worker
        });
        let w2 = tokio::spawn(async move {
            let worker = RussulaBuilder::new(
                BTreeSet::from_iter([w2_sock]),
                NetbenchWorkerProtocol::new(),
            );
            let worker = worker.build().await.unwrap();
            worker.start().await;
            worker
        });

        let c1 = tokio::spawn(async move {
            let addr = BTreeSet::from_iter([w1_sock, w2_sock]);
            let coord = RussulaBuilder::new(addr, NetbenchOrchProtocol::new());
            let coord = coord.build().await.unwrap();
            // assert!(coord.state, Ready)
            // do something
            // assert!(coord.state, WaitPeerDone)
            // assert!(coord.state, Done)
            coord.start().await;
            coord
        });

        let join = tokio::join!(w1, w2, c1);
        let coord = join.2.unwrap();
        coord
            .check_peer_state(NetbenchOrchState::WaitPeerDone)
            .await
            .unwrap();

        assert!(1 == 43)
    }
}
