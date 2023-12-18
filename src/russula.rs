// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

#![allow(unused)]
use crate::russula::protocol::{RussulaPeer, SockProtocol};
use core::{task::Poll, time::Duration};
use std::{collections::BTreeSet, net::SocketAddr};
use tracing::{debug, info, warn};

mod error;
pub mod netbench;
mod network_utils;
mod protocol;

use error::{RussulaError, RussulaResult};
use protocol::{Protocol, StateApi, TransitionStep};

// TODO
// - make state transitions nicer
//   - match on TransitionStep?
// - convert prints to tracing events
//
// D- track peer state for reporting
// D- make notify_peer_done part of the protocol impl
// D- read all queued msg
// D- len for msg
// D- r.transition_step // what is the next step one should take
// D- r.poll_state // take steps to go to next step if possible
// D- handle coord retry on connect
// - should poll current step until all peers are on next step
//   - need api to ask peer state and track peer state
//
// - look at NTP for synchronization: start_at(time)
// https://statecharts.dev/
// halting problem https://en.wikipedia.org/wiki/Halting_problem

const POLL_RETRY_DURATION: Duration = Duration::from_secs(10);

pub struct Russula<P: Protocol> {
    // TODO rename from peer->worker/coord because 'peer' can be confusing
    peer_list: Vec<RussulaPeer<P>>,
    poll_delay: Duration,
}

impl<P: Protocol + Send> Russula<P> {
    pub async fn run_till_ready(&mut self) {
        for peer in self.peer_list.iter_mut() {
            loop {
                match peer.protocol.poll_ready(&peer.stream).await.unwrap() {
                    Poll::Ready(_) => break,
                    Poll::Pending => debug!("{} not yet ready", peer.protocol.name()),
                }
                tokio::time::sleep(self.poll_delay).await;
            }
        }
    }

    pub async fn run_till_state(&mut self, state: P::State) -> RussulaResult<()> {
        while self.poll_state(state).await?.is_pending() {
            tokio::time::sleep(self.poll_delay).await;
        }

        Ok(())
    }

    pub async fn poll_state(&mut self, state: P::State) -> RussulaResult<Poll<()>> {
        for peer in self.peer_list.iter_mut() {
            if let Err(err) = peer.protocol.poll_state(&peer.stream, &state).await {
                if err.is_fatal() {
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

    pub fn self_state_matches(&self, state: P::State) -> bool {
        for peer in self.peer_list.iter() {
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
    peer_list: Vec<SockProtocol<P>>,
    poll_delay: Duration,
}

impl<P: Protocol> RussulaBuilder<P> {
    pub fn new(addr: BTreeSet<SocketAddr>, protocol: P) -> Self {
        let mut map = Vec::new();
        addr.into_iter().for_each(|addr| {
            map.push((addr, protocol.clone()));
        });
        Self {
            peer_list: map,
            poll_delay: POLL_RETRY_DURATION,
        }
    }

    pub fn poll_delay(mut self, delay: Duration) -> Self {
        self.poll_delay = delay;
        self
    }

    pub async fn build(self) -> RussulaResult<Russula<P>> {
        let mut stream_protocol_list = Vec::new();
        for (addr, protocol) in self.peer_list.into_iter() {
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
                            "Failed to connect.. retry attempt left {}. addr: {} dbg: {}",
                            retry_attempts, addr, err
                        );
                        tokio::time::sleep(self.poll_delay).await;
                    }
                }
                retry_attempts -= 1
            }

            info!("Coordinator: successfully connected to {}", addr);
            stream_protocol_list.push(RussulaPeer {
                addr,
                stream,
                protocol,
            });
        }

        Ok(Russula {
            peer_list: stream_protocol_list,
            poll_delay: self.poll_delay,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::russula::netbench::{client, server};
    use std::str::FromStr;

    #[tokio::test]
    async fn netbench_server_protocol() {
        env_logger::init();
        let w1_sock = SocketAddr::from_str("127.0.0.1:8991").unwrap();
        let w2_sock = SocketAddr::from_str("127.0.0.1:8992").unwrap();
        let worker_list = [w1_sock, w2_sock];

        // start the coordinator first and test that the initial `protocol.connect`
        // attempt is retried
        let c1 = tokio::spawn(async move {
            let addr = BTreeSet::from_iter(worker_list);
            let coord = RussulaBuilder::new(addr, server::CoordProtocol::new());
            let mut coord = coord
                .poll_delay(Duration::from_secs(1))
                .build()
                .await
                .unwrap();
            coord.run_till_ready().await;
            coord
        });

        let w1 = tokio::spawn(async move {
            let worker = RussulaBuilder::new(
                BTreeSet::from_iter([w1_sock]),
                server::WorkerProtocol::new(w1_sock.port()),
            );
            let mut worker = worker
                .poll_delay(Duration::from_secs(1))
                .build()
                .await
                .unwrap();
            worker
                .run_till_state(server::WorkerState::Done)
                .await
                .unwrap();
            worker
        });
        let w2 = tokio::spawn(async move {
            let worker = RussulaBuilder::new(
                BTreeSet::from_iter([w2_sock]),
                server::WorkerProtocol::new(w2_sock.port()),
            );
            let mut worker = worker
                .poll_delay(Duration::from_secs(1))
                .build()
                .await
                .unwrap();
            worker
                .run_till_state(server::WorkerState::Done)
                .await
                .unwrap();
            worker
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
        while coord
            .poll_state(server::CoordState::Done)
            .await
            .unwrap()
            .is_pending()
        {
            println!("\npoll state: Done");
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        println!("\nSTEP 20 --------------- : confirm worker done");
        {
            let (worker1, worker2) = tokio::join!(w1, w2);
            let worker1 = worker1.unwrap();
            let worker2 = worker2.unwrap();
            assert!(worker1.self_state_matches(server::WorkerState::Done));
            assert!(worker2.self_state_matches(server::WorkerState::Done));
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
            let coord = RussulaBuilder::new(addr, client::CoordProtocol::new());
            let mut coord = coord
                .poll_delay(Duration::from_secs(1))
                .build()
                .await
                .unwrap();
            coord.run_till_ready().await;
            coord
        });

        let w1 = tokio::spawn(async move {
            let worker = RussulaBuilder::new(
                BTreeSet::from_iter([w1_sock]),
                client::WorkerProtocol::new(w1_sock.port()),
            );
            let mut worker = worker
                .poll_delay(Duration::from_secs(1))
                .build()
                .await
                .unwrap();
            worker
                .run_till_state(client::WorkerState::Done)
                .await
                .unwrap();
            worker
        });
        let w2 = tokio::spawn(async move {
            let worker = RussulaBuilder::new(
                BTreeSet::from_iter([w2_sock]),
                client::WorkerProtocol::new(w2_sock.port()),
            );
            let mut worker = worker
                .poll_delay(Duration::from_secs(1))
                .build()
                .await
                .unwrap();
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
        while coord
            .poll_state(client::CoordState::Done)
            .await
            .unwrap()
            .is_pending()
        {
            println!("\npoll state: Done");
            tokio::time::sleep(Duration::from_secs(1)).await;
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
