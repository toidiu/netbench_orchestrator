// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::protocol::{RussulaPeer, SockProtocol};
use std::{collections::BTreeSet, net::SocketAddr};

mod error;
mod netbench_server_coord;
mod netbench_server_worker;
mod network_utils;
mod protocol;
mod state_action;
mod wip_netbench_server;

use error::{RussulaError, RussulaResult};
use protocol::Protocol;

use self::protocol::{RussulaPoll, StateApi, TransitionStep};

// TODO
// - make state transitions nicer..
//
// - look at NTP for synchronization: start_at(time)
// - handle coord retry on connect
// D- move connect to protocol impl
// https://statecharts.dev/
// halting problem https://en.wikipedia.org/wiki/Halting_problem

pub struct Russula<P: Protocol> {
    peer_list: Vec<RussulaPeer<P>>,
}

impl<P: Protocol + Send> Russula<P> {
    pub async fn run_till_ready(&mut self) {
        for peer in self.peer_list.iter_mut() {
            peer.protocol.run_till_ready(&peer.stream).await.unwrap();
        }
    }

    pub async fn run_till_done(&mut self) {
        for peer in self.peer_list.iter_mut() {
            peer.protocol.run_till_done(&peer.stream).await.unwrap();
        }
    }

    pub async fn poll_state(&mut self, state: P::State) -> RussulaPoll {
        for peer in self.peer_list.iter_mut() {
            // poll till state and break if Pending
            while !peer.protocol.state().eq(&state) {
                let poll = peer.protocol.poll_state(&peer.stream, state).await.unwrap();
                if let RussulaPoll::Pending(p) = poll {
                    return RussulaPoll::Pending(p);
                }
            }
        }
        RussulaPoll::Ready
    }

    pub async fn check_self_state(&self, state: P::State) -> RussulaResult<bool> {
        let mut matches = true;
        for peer in self.peer_list.iter() {
            let protocol_state = peer.protocol.state();
            matches &= state.eq(protocol_state);
            // println!("{:?} {:?} {}", protocol_state, state, matches);
        }
        Ok(matches)
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
    use crate::russula::{
        netbench_server_coord::{CoordNetbenchServerState, NetbenchCoordServerProtocol},
        netbench_server_worker::{NetbenchWorkerServerProtocol, WorkerNetbenchServerState},
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
            worker.run_till_ready().await;
            worker
        });
        let w2 = tokio::spawn(async move {
            let worker = RussulaBuilder::new(
                BTreeSet::from_iter([w2_sock]),
                NetbenchWorkerServerProtocol::new(),
            );
            let mut worker = worker.build().await.unwrap();
            worker.run_till_ready().await;
            worker
        });

        let c1 = tokio::spawn(async move {
            let addr = BTreeSet::from_iter([w1_sock, w2_sock]);
            let coord = RussulaBuilder::new(addr, NetbenchCoordServerProtocol::new());
            let mut coord = coord.build().await.unwrap();
            coord.run_till_ready().await;
            coord
        });

        let join = tokio::join!(w1, w2, c1);

        let mut worker1 = join.0.unwrap();
        assert!(worker1
            .check_self_state(WorkerNetbenchServerState::Ready)
            .await
            .unwrap());
        let mut worker2 = join.1.unwrap();
        assert!(worker2
            .check_self_state(WorkerNetbenchServerState::Ready)
            .await
            .unwrap());

        let mut coord = join.2.unwrap();
        assert!(coord
            .check_self_state(CoordNetbenchServerState::Ready)
            .await
            .unwrap());

        assert!(matches!(
            coord.poll_state(CoordNetbenchServerState::Ready).await,
            RussulaPoll::Ready
        ));

        assert!(matches!(
            coord.poll_state(CoordNetbenchServerState::RunPeer).await,
            RussulaPoll::Ready
        ));

        // FIXME need to return Poll and run in loop
        // coord.run_till_done().await;
        // worker1.run_till_done().await;
        // worker2.run_till_done().await;

        // assert!(coord
        //     .check_self_state(CoordNetbenchServerState::Done)
        //     .await
        //     .unwrap());

        assert!(21 == 22);
    }
}
