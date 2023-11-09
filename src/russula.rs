// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::protocol::{RussulaPeer, SockProtocol};
use core::{task::Poll, time::Duration};
use std::{collections::BTreeSet, net::SocketAddr};

mod error;
mod netbench;
mod network_utils;
mod protocol;
mod state_action;

use error::{RussulaError, RussulaResult};
use protocol::{Protocol, StateApi, TransitionStep};

// TODO
// - make state transitions nicer
//   - match on TransitionStep?
//
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

pub struct Russula<P: Protocol> {
    peer_list: Vec<RussulaPeer<P>>,
}

impl<P: Protocol + Send> Russula<P> {
    pub async fn run_till_ready(&mut self) {
        for peer in self.peer_list.iter_mut() {
            loop {
                match peer.protocol.poll_ready(&peer.stream).await.unwrap() {
                    Poll::Ready(_) => break,
                    Poll::Pending => println!("{} not yet ready", peer.protocol.name()),
                }
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    }

    pub async fn poll_next(&mut self) -> RussulaResult<Poll<()>> {
        for peer in self.peer_list.iter_mut() {
            // poll till state and break if Pending
            let poll = peer.protocol.poll_next(&peer.stream).await?;
            if poll.is_pending() {
                return Ok(Poll::Pending);
            }
        }
        Ok(Poll::Ready(()))
    }

    pub async fn notify_peer_done(&mut self) -> RussulaResult<()> {
        for peer in self.peer_list.iter_mut() {
            if !peer.protocol.is_done_state() {
                return Err(RussulaError::Usage {
                    dbg: format!(
                        "{} not in done state. current_state: {:?}",
                        peer.protocol.name(),
                        peer.protocol.state()
                    ),
                });
            }
            peer.protocol.run_current(&peer.stream).await?;
        }

        Ok(())
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

    pub fn transition_step(&mut self) -> Vec<TransitionStep> {
        let mut steps = Vec::new();
        for peer in self.peer_list.iter() {
            let step = peer.protocol.state().transition_step();
            steps.push(step);
        }
        steps
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
            let stream;
            loop {
                match protocol.connect(&addr).await {
                    Ok(connect) => {
                        stream = connect;
                        break;
                    }
                    Err(RussulaError::NetworkConnectionRefused { dbg }) => {
                        println!("Failed to connect.. retrying. addr: {} dbg: {}", addr, dbg);
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                    Err(err) => return Err(err),
                }
            }
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
    use crate::russula::netbench::server;
    use core::time::Duration;
    use std::str::FromStr;

    #[tokio::test]
    #[allow(clippy::assertions_on_constants)] // for testing
    async fn netbench_server_protocol() {
        let w1_sock = SocketAddr::from_str("127.0.0.1:8991").unwrap();
        let w2_sock = SocketAddr::from_str("127.0.0.1:8992").unwrap();
        let worker_list = [w1_sock, w2_sock];

        // start the coordinator first to test initial connection retry
        let c1 = tokio::spawn(async move {
            let addr = BTreeSet::from_iter(worker_list);
            let coord = RussulaBuilder::new(addr, server::CoordProtocol::new());
            let mut coord = coord.build().await.unwrap();
            coord.run_till_ready().await;
            coord
        });

        let w1 = tokio::spawn(async move {
            let worker = RussulaBuilder::new(
                BTreeSet::from_iter([w1_sock]),
                server::WorkerProtocol::new(w1_sock.port()),
            );
            let mut worker = worker.build().await.unwrap();
            while !worker
                .check_self_state(server::WorkerState::Done)
                .await
                .unwrap()
            {
                println!("[worker-1] run-------looooooooooop---------");
                let _ = worker.poll_next().await;
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            worker
        });
        let w2 = tokio::spawn(async move {
            let worker = RussulaBuilder::new(
                BTreeSet::from_iter([w2_sock]),
                server::WorkerProtocol::new(w2_sock.port()),
            );
            let mut worker = worker.build().await.unwrap();
            while !worker
                .check_self_state(server::WorkerState::Done)
                .await
                .unwrap()
            {
                println!("[worker-2] run-------looooooooooop---------");
                let _ = worker.poll_next().await;
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            worker
        });

        let join = tokio::join!(c1);
        let mut coord = join.0.unwrap();

        println!("\nSTEP 1 --------------- : confirm current ready state");
        // we are already in the Ready state
        {
            assert!(coord
                .check_self_state(server::CoordState::Ready)
                .await
                .unwrap());
        }

        println!("\nSTEP 3 --------------- : poll next coord step");
        // move coord forward
        {
            while !coord
                .check_self_state(server::CoordState::RunPeer)
                .await
                .unwrap()
            {
                if let Err(err) = coord.poll_next().await {
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    if err.is_fatal() {
                        panic!("{}", err);
                    }
                }
            }
        }

        let delay_kill = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(3)).await;
            println!("\nSTEP 4 --------------- : sleep and then kill worker");
            // move coord forward
            {
                while !coord
                    .check_self_state(server::CoordState::Done)
                    .await
                    .unwrap()
                {
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    if let Err(err) = coord.poll_next().await {
                        if err.is_fatal() {
                            panic!("{}", err);
                        }
                    }
                }

                // Notify peer that coordinator is done. This is best effort
                for _i in 0..3 {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    let _ignore_error = coord.notify_peer_done().await;
                }
                // panic!("{}", "-----------------------------1-1-1-1--1-1-1b;a");
            }
        });

        let join = tokio::join!(delay_kill);
        join.0.unwrap();

        println!("\nSTEP 20 --------------- : confirm worker done");
        {
            let (worker1, worker2) = tokio::join!(w1, w2);
            let worker1 = worker1.unwrap();
            let worker2 = worker2.unwrap();
            assert!(worker1
                .check_self_state(server::WorkerState::Done)
                .await
                .unwrap());
            assert!(worker2
                .check_self_state(server::WorkerState::Done)
                .await
                .unwrap());
        }

        assert!(22 == 20, "\n\n\nSUCCESS ---------------- INTENTIONAL FAIL");
    }
}
