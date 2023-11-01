// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::protocol::SockProtocol;
use std::{collections::BTreeSet, net::SocketAddr};
use tokio::net::TcpStream;

mod error;
mod netbench_server_coord;
mod netbench_server_worker;
mod protocol;
mod wip_netbench_server;

use error::RussulaResult;
use protocol::Protocol;

use self::protocol::NextTransitionMsg;
use self::protocol::StateApi;

// TODO
// - add PeerState type to Protocol
// - loop over send/recv
// - ack state update to peer
// - make state transitions nicer..
//
// - handle coord retry on connect
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
    pub async fn start(&mut self) {
        for peer in self.peer_list.iter_mut() {
            peer.protocol.start(&peer.stream).await.unwrap();
        }
    }

    #[allow(unused_variables)]
    pub async fn check_self_state(&self, state: P::State) -> RussulaResult<bool> {
        let mut matches = true;
        for peer in self.peer_list.iter() {
            let protocol_state = peer.protocol.state();
            matches &= state.eq(protocol_state);
            println!("{:?} {:?} {}", protocol_state, state, matches);
        }
        Ok(matches)
    }

    #[allow(unused_variables)]
    pub async fn check_peer_state(&self, state: P::State) -> RussulaResult<bool> {
        todo!()
    }
}

pub struct RussulaBuilder<P: Protocol> {
    peer_list: Vec<SockProtocol<P>>,
}

impl<P: Protocol> RussulaBuilder<P> {
    pub fn new(addr: BTreeSet<SocketAddr>, protocol: P) -> Self {
        let mut map = Vec::new();
        addr.into_iter().for_each(|addr| {
            map.push((addr, protocol.clone()));
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
    use crate::russula::netbench_server_coord::{
        NetbenchCoordServerProtocol, NetbenchCoordServerState,
    };
    use crate::russula::netbench_server_worker::{
        NetbenchWorkerServerProtocol, NetbenchWorkerServerState,
    };
    use std::str::FromStr;

    #[tokio::test]
    async fn russula_netbench() {
        let w1_sock = SocketAddr::from_str("127.0.0.1:8991").unwrap();
        let w2_sock = SocketAddr::from_str("127.0.0.1:8993").unwrap();

        let w1 = tokio::spawn(async move {
            let worker = RussulaBuilder::new(
                BTreeSet::from_iter([w1_sock]),
                NetbenchWorkerServerProtocol::new(),
            );
            let mut worker = worker.build().await.unwrap();
            worker.start().await;
            worker
        });
        let w2 = tokio::spawn(async move {
            let worker = RussulaBuilder::new(
                BTreeSet::from_iter([w2_sock]),
                NetbenchWorkerServerProtocol::new(),
            );
            let mut worker = worker.build().await.unwrap();
            worker.start().await;
            worker
        });

        let c1 = tokio::spawn(async move {
            let addr = BTreeSet::from_iter([w1_sock, w2_sock]);
            let coord = RussulaBuilder::new(addr, NetbenchCoordServerProtocol::new());
            let mut coord = coord.build().await.unwrap();
            // assert!(coord.state, Ready)
            // do something
            // assert!(coord.state, WaitPeerDone)
            // assert!(coord.state, Done)
            coord.start().await;
            coord
        });

        let join = tokio::join!(w1, w2, c1);

        let worker1 = join.0.unwrap();
        assert!(worker1
            .check_self_state(NetbenchWorkerServerState::ServerReady)
            .await
            .unwrap());
        let worker2 = join.1.unwrap();
        assert!(worker2
            .check_self_state(NetbenchWorkerServerState::ServerReady)
            .await
            .unwrap());

        let coord = join.2.unwrap();
        assert!(coord
            .check_self_state(NetbenchCoordServerState::CoordWaitPeerDone)
            .await
            .unwrap());
    }
}
