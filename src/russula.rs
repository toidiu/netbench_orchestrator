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
//
// - curr_state ()
// - peer_state_update (to, from)
//   - ack msg (send curr state)
// - self_state_updated (to, from)
//
// - worker groups (server, client)
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
            Role::Coordinator(worker_list) => {
                let mut v = Vec::new();
                for (addr, protocol) in worker_list.iter() {
                    let stream = protocol.connect_to_worker(*addr).await?;
                    println!("Coordinator: successfully connected to {}", addr);
                    v.push((stream, protocol));
                }

                for (stream, protocol) in v.into_iter() {
                    // TODO start instead
                    protocol.send_msg(stream, protocol.state()).await?;
                }
            }
            Role::Worker((addr, protocol)) => {
                let stream = protocol.wait_for_coordinator(addr).await?;

                // TODO start instead
                protocol.recv_msg(stream).await?;
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
            Role::Worker((_addr, protocol)) => protocol.kill(),
        }
    }

    #[allow(unused_variables)]
    pub async fn is_peer_state(&self, state: P::State) -> RussulaResult<bool> {
        let matches = match &self.role {
            Role::Coordinator(map) => {
                let mut matches = true;
                for (_addr, protocol) in map.iter() {
                    let protocol_state = protocol.peer_state();
                    matches &= matches!(state, protocol_state);
                }
                matches
            }
            Role::Worker((_addr, protocol)) => {
                let protocol_state = protocol.peer_state();
                matches!(state, protocol_state)
            }
        };
        Ok(matches)
    }
}

#[derive(Clone, Copy)]
pub struct NetbenchProtocol {
    state: NetbenchState,
    peer_state: NetbenchState,
}

impl NetbenchProtocol {
    pub fn new() -> Self {
        NetbenchProtocol {
            state: NetbenchState::Ready,
            peer_state: NetbenchState::Ready,
        }
    }
}

#[async_trait]
impl Protocol for NetbenchProtocol {
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

    async fn connect_to_worker(&self, addr: SocketAddr) -> RussulaResult<TcpStream> {
        println!("--- Coordinator: attempt to connect to worker on: {}", addr);

        let connect = TcpStream::connect(addr)
            .await
            .map_err(|err| RussulaError::Connect {
                dbg: err.to_string(),
            })?;

        Ok(connect)
    }

    async fn recv_msg(&self, stream: TcpStream) -> RussulaResult<Self::State> {
        stream.readable().await.unwrap();

        let mut buf = Vec::with_capacity(100);
        match stream.try_read_buf(&mut buf) {
            Ok(n) => {
                let msg = NetbenchState::from_bytes(&buf)?;
                println!("read {} bytes: {:?}", n, &msg);
            }
            Err(ref e) if e.kind() == tokio::io::ErrorKind::WouldBlock => {
                panic!("{}", e)
            }
            Err(e) => panic!("{}", e),
        }

        // TODO
        Ok(self.state)
    }

    async fn send_msg(&self, stream: TcpStream, msg: Self::State) -> RussulaResult<()> {
        stream.writable().await.unwrap();

        stream.try_write(msg.as_bytes()).unwrap();

        Ok(())
    }

    fn state(&self) -> Self::State {
        self.state
    }
    fn peer_state(&self) -> Self::State {
        self.peer_state
    }
}

//  curr_state                self/peer driven       notify peer of curr state          fn to go to next
//
//  Ready(Ip),                Some("ready_next"),    false,                             Running((Ip, TcpStream))
//  Running((Ip, TcpStream)), None,                  true,                              Done((Ip, TcpStream))
//
// A("name",                  Option(MSG_to_next),   Notify_peer_of_transition_to_next, Fn(Self)->Self )
// B("name",                  Option(MSG_to_next),   Notify_peer_of_transition_to_next)
#[derive(Copy, Clone, Debug)]
pub enum NetbenchState {
    Ready,
    Run,
    Done,
}

impl NetbenchState {
    pub fn as_bytes(&self) -> &'static [u8] {
        match self {
            NetbenchState::Ready => b"ready",
            NetbenchState::Run => b"run",
            NetbenchState::Done => b"done",
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> RussulaResult<Self> {
        let state = match bytes {
            b"ready" => NetbenchState::Ready,
            b"run" => NetbenchState::Run,
            b"done" => NetbenchState::Done,
            bad_msg => {
                return Err(RussulaError::BadMsg {
                    dbg: format!("unrecognized msg {:?}", bad_msg),
                })
            }
        };

        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[tokio::test]
    async fn test() {
        let test_protocol = NetbenchProtocol::new();

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
        coord.is_peer_state(NetbenchState::Run).await.unwrap();
        coord.kill().await;

        assert!(1 == 43)
    }
}
