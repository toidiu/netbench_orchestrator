// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

#![allow(unused)]
use crate::russula::protocol::{ProtocolInstance, SockProtocol};
use core::{task::Poll, time::Duration};
use std::{collections::BTreeSet, net::SocketAddr};
use tracing::{debug, error, info, warn};

mod error;
pub mod netbench;
mod network_utils;
mod protocol;
mod states;

use error::{RussulaError, RussulaResult};
use protocol::Protocol;
use states::{StateApi, TransitionStep};

// TODO
// - make state transitions nicer
//   - match on TransitionStep?
// - convert prints to tracing events
//
// - should poll current step until all peers are on next step
//   - need api to ask peer state and track peer state
//
// - look at NTP for synchronization: start_at(time)
// https://statecharts.dev/
// halting problem https://en.wikipedia.org/wiki/Halting_problem

pub struct Russula<P: Protocol> {
    // Protocol instances part of this Russula Coordinator/Worker.
    //
    // The Coord should be a list of size 1
    // The Worker can be list of size >=1
    instance_list: Vec<ProtocolInstance<P>>,
    poll_delay: Duration,
    protocol: P,
}

impl<P: Protocol + Send> Russula<P> {
    pub async fn run_till_ready(&mut self) -> RussulaResult<()> {
        let ready_state = self.protocol.ready_state();
        self.run_till_state(ready_state).await
    }

    pub async fn poll_done(&mut self) -> RussulaResult<Poll<()>> {
        let done_state = self.protocol.done_state();
        self.poll_state(done_state).await
    }

    pub async fn poll_state(&mut self, state: P::State) -> RussulaResult<Poll<()>> {
        for peer in self.instance_list.iter_mut() {
            if let Err(err) = peer.protocol.poll_state(&peer.stream, &state).await {
                if err.is_fatal() {
                    error!("{}", err);
                    panic!("{}", err);
                }
            }
        }
        let poll = if self.self_state_matches(state) {
            Poll::Ready(())
        } else {
            Poll::Pending
        };
        Ok(poll)
    }

    pub(crate) async fn run_till_state(&mut self, state: P::State) -> RussulaResult<()> {
        while self.poll_state(state).await?.is_pending() {
            tokio::time::sleep(self.poll_delay).await;
        }

        Ok(())
    }

    fn self_state_matches(&self, state: P::State) -> bool {
        for peer in self.instance_list.iter() {
            let protocol_state = peer.protocol.state();
            if !state.eq(protocol_state) {
                return false;
            }
            // info!("{:?} {:?} {}", protocol_state, state, matches);
        }
        true
    }
}

pub struct RussulaBuilder<P: Protocol> {
    // Address for the Coordinator and Worker to communicate on.
    //
    // The Coordinator gets a list of workers addrs to 'connect' to.
    // The Worker gets its own addr to 'listen' on.
    russula_pair_addr_list: Vec<SockProtocol<P>>,
    poll_delay: Duration,
    protocol: P,
}

impl<P: Protocol> RussulaBuilder<P> {
    pub fn new(peer_addr: BTreeSet<SocketAddr>, protocol: P, poll_delay: Duration) -> Self {
        // TODO if worker check that the list is len 1 and points to local addr on which to listen
        let mut peer_list = Vec::new();
        peer_addr.into_iter().for_each(|addr| {
            peer_list.push((addr, protocol.clone()));
        });
        Self {
            russula_pair_addr_list: peer_list,
            poll_delay,
            protocol,
        }
    }

    pub async fn build(self) -> RussulaResult<Russula<P>> {
        let mut stream_protocol_list = Vec::new();
        for (addr, protocol) in self.russula_pair_addr_list.into_iter() {
            let stream;
            let mut retry_attempts = 3;
            loop {
                if retry_attempts == 0 {
                    return Err(RussulaError::NetworkConnectionRefused {
                        dbg: "Failed to connect to peer".to_string(),
                    });
                }
                match protocol.connect(&addr).await {
                    Ok(connect) => {
                        stream = connect;
                        break;
                    }
                    Err(err) => {
                        warn!(
                            "Failed to connect.. waiting before retrying. Retry attempts left: {}. addr: {} dbg: {}",
                            retry_attempts, addr, err
                        );
                        warn!("Try disabling VPN and check your network connectivity");
                        tokio::time::sleep(self.poll_delay).await;
                    }
                }
                retry_attempts -= 1
            }

            info!("Coordinator: successfully connected to {}", addr);
            stream_protocol_list.push(ProtocolInstance {
                addr,
                stream,
                protocol,
            });
        }

        Ok(Russula {
            instance_list: stream_protocol_list,
            poll_delay: self.poll_delay,
            protocol: self.protocol,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::russula::netbench::{client, server};
    use futures::future::join_all;
    use std::str::FromStr;

    const POLL_DELAY_DURATION: Duration = Duration::from_secs(1);

    #[tokio::test]
    async fn netbench_server_protocol() {
        env_logger::init();

        let mut worker_addrs = Vec::new();
        let mut workers = Vec::new();
        macro_rules! worker {
            {$worker:ident, $sock:ident, $port:literal} => {
                let $sock = SocketAddr::from_str(&format!("127.0.0.1:{}", $port)).unwrap();
                let $worker = tokio::spawn(async move {
                    let worker = RussulaBuilder::new(
                        BTreeSet::from_iter([$sock]),
                        server::WorkerProtocol::new(
                            $sock.port().to_string(),
                            netbench::ServerContext::testing(),
                        ),
                        POLL_DELAY_DURATION,
                    );
                    let mut worker = worker.build().await.unwrap();
                    worker
                        .run_till_state(server::WorkerState::Done)
                        .await
                        .unwrap();
                    worker
                });

                workers.push($worker);
                worker_addrs.push($sock);
            };
        }

        worker!(w1, w1_sock, 9001);
        worker!(w2, w2_sock, 9002);
        worker!(w3, w3_sock, 9003);
        worker!(w4, w4_sock, 9004);
        worker!(w5, w5_sock, 9005);
        worker!(w6, w6_sock, 9006);
        worker!(w7, w7_sock, 9007);

        // start the coordinator first and test that the initial `protocol.connect`
        // attempt is retried
        let c1 = tokio::spawn(async move {
            let addr = BTreeSet::from_iter(worker_addrs);
            let protocol = server::CoordProtocol::new();
            let coord = RussulaBuilder::new(addr, protocol, POLL_DELAY_DURATION);
            let mut coord = coord.build().await.unwrap();
            coord.run_till_ready().await.unwrap();
            coord
        });

        let join = tokio::join!(c1);
        let mut coord = join.0.unwrap();

        println!("\nSTEP 1 --------------- : confirm current ready state");
        {
            assert!(coord.self_state_matches(server::CoordState::Ready));
        }

        println!("\nSTEP 2 --------------- : poll next coord step");
        {
            coord
                .run_till_state(server::CoordState::WorkersRunning)
                .await
                .unwrap();
        }

        println!("\nSTEP 3 --------------- : wait till done");
        while coord.poll_done().await.unwrap().is_pending() {
            println!("\npoll state: Done");
            tokio::time::sleep(POLL_DELAY_DURATION).await;
        }

        println!("\nSTEP 20 --------------- : confirm worker done");
        {
            let worker_join = join_all(workers).await;
            for w in worker_join {
                assert!(w.unwrap().self_state_matches(server::WorkerState::Done));
            }
        }
    }

    #[tokio::test]
    async fn netbench_client_protocol() {
        env_logger::init();
        let w1_sock = SocketAddr::from_str("127.0.0.1:9991").unwrap();
        let w2_sock = SocketAddr::from_str("127.0.0.1:9992").unwrap();
        let worker_list = [w1_sock, w2_sock];

        // start the coordinator first and test that the initial `protocol.connect`
        // attempt is retried
        let c1 = tokio::spawn(async move {
            let addr = BTreeSet::from_iter(worker_list);

            let protocol = client::CoordProtocol::new();
            let coord = RussulaBuilder::new(addr, protocol, POLL_DELAY_DURATION);
            let mut coord = coord.build().await.unwrap();
            coord.run_till_ready().await.unwrap();
            coord
        });

        let w1 = tokio::spawn(async move {
            let worker = RussulaBuilder::new(
                BTreeSet::from_iter([w1_sock]),
                client::WorkerProtocol::new(
                    w1_sock.port().to_string(),
                    netbench::ClientContext::testing(),
                ),
                POLL_DELAY_DURATION,
            );
            let mut worker = worker.build().await.unwrap();
            worker
                .run_till_state(client::WorkerState::Done)
                .await
                .unwrap();
            worker
        });
        let w2 = tokio::spawn(async move {
            let worker = RussulaBuilder::new(
                BTreeSet::from_iter([w2_sock]),
                client::WorkerProtocol::new(
                    w2_sock.port().to_string(),
                    netbench::ClientContext::testing(),
                ),
                POLL_DELAY_DURATION,
            );
            let mut worker = worker.build().await.unwrap();
            worker
                .run_till_state(client::WorkerState::Done)
                .await
                .unwrap();
            worker
        });

        let join = tokio::join!(c1);
        let mut coord = join.0.unwrap();

        println!("\nclient-STEP 1 --------------- : confirm current ready state");
        {
            assert!(coord.self_state_matches(client::CoordState::Ready));
        }

        println!("\nclient-STEP 2 --------------- : wait for workers to run");
        {
            coord
                .run_till_state(client::CoordState::WorkersRunning)
                .await
                .unwrap();
        }

        println!("\nSTEP 3 --------------- : wait till done");
        while coord.poll_done().await.unwrap().is_pending() {
            println!("\npoll state: Done");
            tokio::time::sleep(POLL_DELAY_DURATION).await;
        }

        println!("\nclient-STEP 20 --------------- : confirm worker done");
        {
            let (worker1, worker2) = tokio::join!(w1, w2);
            let worker1 = worker1.unwrap();
            let worker2 = worker2.unwrap();
            assert!(worker1.self_state_matches(client::WorkerState::Done));
            assert!(worker2.self_state_matches(client::WorkerState::Done));
        }
    }
}
