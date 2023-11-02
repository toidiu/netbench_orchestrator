// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::netbench_server_coord::NetbenchCoordServerState;
use crate::russula::NextTransitionMsg;
use crate::russula::StateApi;
use async_trait::async_trait;
use bytes::Bytes;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};

use crate::russula::error::{RussulaError, RussulaResult};
use crate::russula::protocol::Protocol;

#[derive(Copy, Clone, Debug)]
pub enum NetbenchWorkerServerState {
    ServerWaitCoordInit,
    ServerReady,
    ServerRun,
    ServerDone,
}

#[derive(Clone, Copy)]
pub struct NetbenchWorkerServerProtocol {
    state: NetbenchWorkerServerState,
}

impl NetbenchWorkerServerProtocol {
    pub fn new() -> Self {
        NetbenchWorkerServerProtocol {
            state: NetbenchWorkerServerState::ServerWaitCoordInit,
        }
    }
}

#[async_trait]
impl Protocol for NetbenchWorkerServerProtocol {
    type State = NetbenchWorkerServerState;

    async fn connect(&self, addr: &SocketAddr) -> RussulaResult<TcpStream> {
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

    async fn start(&mut self, stream: &TcpStream) -> RussulaResult<()> {
        let msg = self.recv_msg(stream).await?;

        self.state.process_msg(msg);

        Ok(())
    }

    async fn recv_msg(&self, stream: &TcpStream) -> RussulaResult<Bytes> {
        stream.readable().await.unwrap();

        let mut buf = Vec::with_capacity(100);
        match stream.try_read_buf(&mut buf) {
            Ok(_n) => Ok(Bytes::from_iter(buf)),
            Err(ref e) if e.kind() == tokio::io::ErrorKind::WouldBlock => {
                panic!("{}", e)
            }
            Err(e) => panic!("{}", e),
        }

        // TODO
        // Ok(self.state)
    }

    async fn send_msg(&self, stream: &TcpStream, msg: Self::State) -> RussulaResult<()> {
        stream.writable().await.unwrap();

        stream.try_write(msg.as_bytes()).unwrap();

        Ok(())
    }

    fn state(&self) -> Self::State {
        self.state
    }
}

impl StateApi for NetbenchWorkerServerState {
    fn eq(&self, other: Self) -> bool {
        match self {
            NetbenchWorkerServerState::ServerWaitCoordInit => {
                matches!(other, NetbenchWorkerServerState::ServerWaitCoordInit)
            }
            NetbenchWorkerServerState::ServerReady => {
                matches!(other, NetbenchWorkerServerState::ServerReady)
            }
            NetbenchWorkerServerState::ServerRun => {
                matches!(other, NetbenchWorkerServerState::ServerRun)
            }
            NetbenchWorkerServerState::ServerDone => {
                matches!(other, NetbenchWorkerServerState::ServerDone)
            }
        }
    }

    fn expect_peer_msg(&self) -> Option<NextTransitionMsg> {
        match self {
            NetbenchWorkerServerState::ServerWaitCoordInit => Some(NextTransitionMsg::PeerDriven(
                NetbenchCoordServerState::CoordCheckPeer.as_bytes(),
            )),
            NetbenchWorkerServerState::ServerReady => Some(NextTransitionMsg::PeerDriven(
                // FIXME
                // NetbenchCoordServerState::CoordRunPeer.as_bytes(),
                NetbenchCoordServerState::CoordCheckPeer.as_bytes(),
            )),
            NetbenchWorkerServerState::ServerRun => Some(NextTransitionMsg::PeerDriven(
                // FIXME
                // NetbenchCoordServerState::CoordKillPeer.as_bytes(),
                NetbenchCoordServerState::CoordCheckPeer.as_bytes(),
            )),
            NetbenchWorkerServerState::ServerDone => None,
        }
    }

    fn next(&mut self) {
        let a = match self {
            NetbenchWorkerServerState::ServerWaitCoordInit => {
                NetbenchWorkerServerState::ServerReady
            }
            NetbenchWorkerServerState::ServerReady => NetbenchWorkerServerState::ServerRun,
            NetbenchWorkerServerState::ServerRun => NetbenchWorkerServerState::ServerDone,
            NetbenchWorkerServerState::ServerDone => NetbenchWorkerServerState::ServerDone,
        };
        *self = a;
    }

    fn process_msg(&mut self, msg: Bytes) {
        if let Some(NextTransitionMsg::PeerDriven(peer_msg)) = self.expect_peer_msg() {
            println!(
                "worker {:?} {:?}",
                std::str::from_utf8(peer_msg),
                std::str::from_utf8(&msg)
            );
            if peer_msg == msg {
                self.next();
            }
        }
    }
}

impl NetbenchWorkerServerState {
    pub fn as_bytes(&self) -> &'static [u8] {
        match self {
            NetbenchWorkerServerState::ServerWaitCoordInit => b"server_wait_coord_init",
            NetbenchWorkerServerState::ServerReady => b"server_ready",
            NetbenchWorkerServerState::ServerRun => b"server_wait_peer_done",
            NetbenchWorkerServerState::ServerDone => b"server_done",
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> RussulaResult<Self> {
        let state = match bytes {
            b"server_wait_coord_init" => NetbenchWorkerServerState::ServerWaitCoordInit,
            b"server_ready" => NetbenchWorkerServerState::ServerReady,
            b"server_wait_peer_done" => NetbenchWorkerServerState::ServerRun,
            b"server_done" => NetbenchWorkerServerState::ServerDone,
            bad_msg => {
                return Err(RussulaError::BadMsg {
                    dbg: format!("unrecognized msg {:?}", std::str::from_utf8(bad_msg)),
                })
            }
        };

        Ok(state)
    }
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn netbench_state() {}
}
