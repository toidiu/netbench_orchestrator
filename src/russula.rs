// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;
use std::{collections::BTreeSet, net::SocketAddr};
use tokio::net::{TcpListener, TcpStream};

mod error;
mod protocol;

use error::{RussulaError, RussulaResult};
use protocol::Protocol;
use protocol::Role;

pub struct Russula<P: Protocol> {
    role: Role<P>,
}

// TODO
// - handle coord retry on connect
// D- move connect to protocol impl

impl<P: Protocol> Russula<P> {
    pub fn new_coordinator(addr: BTreeSet<SocketAddr>, protocol: P) -> Self {
        let mut map = Vec::new();
        addr.into_iter().for_each(|addr| {
            map.push((addr, protocol.clone()));
        });
        let role = Role::Coordinator(map);
        Self { role }
    }

    pub fn new_worker(addr: SocketAddr, protocol: P) -> Self {
        Self {
            role: Role::Worker((addr, protocol)),
        }
    }

    pub async fn connect(&self) -> RussulaResult<()> {
        match &self.role {
            Role::Coordinator(protocol_map) => {
                for (addr, protocol) in protocol_map.iter() {
                    protocol.connect_to_worker(*addr).await;
                }
            }
            Role::Worker((addr, protocol)) => {
                let stream = protocol.wait_for_coordinator(addr).await?;

                // TODO move to protocol.start
                protocol.recv_msg(stream).await;
            }
        }

        Ok(())
    }

    pub async fn start(&self) {
        match &self.role {
            Role::Coordinator(map) => {
                for (_addr, protocol) in map.iter() {
                    protocol.start();
                }
            }
            Role::Worker((_addr, protocol)) => protocol.start(),
        }
    }

    pub async fn kill(&self) {
        match &self.role {
            Role::Coordinator(map) => {
                for (_addr, protocol) in map.iter() {
                    protocol.kill();
                }
            }
            Role::Worker(role) => role.1.kill(),
        }
    }

    pub async fn wait_peer_state(&self, _state: P::State) {}
}

#[derive(Clone, Copy)]
pub struct NetbenchOrchestrator {
    state: NetbenchState,
    peer_state: NetbenchState,
}

impl NetbenchOrchestrator {
    pub fn new() -> Self {
        NetbenchOrchestrator {
            state: NetbenchState::Ready,
            peer_state: NetbenchState::Ready,
        }
    }
}

#[async_trait]
impl Protocol for NetbenchOrchestrator {
    type State = NetbenchState;

    async fn wait_for_coordinator(&self, addr: &SocketAddr) -> RussulaResult<TcpStream> {
        let listener = TcpListener::bind(addr).await.unwrap();
        println!("--- Worker listening on: {}", addr);

        let (stream, _local_addr) =
            listener
                .accept()
                .await
                .map_err(|err| RussulaError::Connect {
                    dbg: err.to_string(),
                })?;
        println!("Worker success connection: {addr}");

        Ok(stream)
    }

    async fn connect_to_worker(&self, addr: SocketAddr) {
        println!("--- Coordinator: attempt to connect to worker on: {}", addr);

        let connect = TcpStream::connect(addr);
        match connect.await {
            Ok(stream) => {
                println!("Coordinator: successfully connected to {}", addr);
                self.send_msg(stream, self.state).await;
            }
            Err(_) => println!("failed to connect to worker {}", addr),
        }
    }

    async fn recv_msg(&self, stream: TcpStream) -> Self::State {
        stream.readable().await.unwrap();

        let mut buf = Vec::with_capacity(4096);
        match stream.try_read_buf(&mut buf) {
            Ok(n) => {
                let msg = std::str::from_utf8(&buf);
                println!("read {} bytes: {:?}", n, &msg);
            }
            Err(ref e) if e.kind() == tokio::io::ErrorKind::WouldBlock => {
                panic!("{}", e)
            }
            Err(e) => panic!("{}", e),
        }

        self.state
    }

    async fn send_msg(&self, stream: TcpStream, msg: Self::State) {
        stream.writable().await.unwrap();

        let msg = format!("hi {:?}", msg);
        stream.try_write(msg.as_bytes()).unwrap();
    }

    fn peer_state(&self) -> Self::State {
        self.peer_state
    }
}

#[derive(Copy, Clone, Debug)]
pub enum NetbenchState {
    Ready,
    Run,
    Done,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[tokio::test]
    async fn test() {
        let test_protocol = NetbenchOrchestrator::new();

        let w1_sock = SocketAddr::from_str("127.0.0.1:8991").unwrap();
        let w2_sock = SocketAddr::from_str("127.0.0.1:8992").unwrap();

        let w1 = tokio::spawn(async move {
            let worker = Russula::new_worker(w1_sock, test_protocol);
            worker.connect().await.unwrap();
            worker
        });
        let w2 = tokio::spawn(async move {
            let worker = Russula::new_worker(w2_sock, test_protocol);
            worker.connect().await.unwrap();
            worker
        });

        let c1 = tokio::spawn(async move {
            let addr = BTreeSet::from_iter([w1_sock, w2_sock]);
            let coord = Russula::new_coordinator(addr, test_protocol);
            coord.connect().await.unwrap();
            coord
        });

        let join = tokio::join!(w1, w2, c1);
        let coord = join.2.unwrap();
        coord.kill().await;

        assert!(1 == 43)
    }
}
