// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::netbench_server::NetbenchWorkerServerState;
use crate::russula::NextTransitionMsg;
use crate::russula::StateApi;
use async_trait::async_trait;
use bytes::Bytes;
use std::net::SocketAddr;
use tokio::net::TcpStream;

use crate::russula::error::{RussulaError, RussulaResult};
use crate::russula::protocol::Protocol;

#[derive(Clone, Copy)]
pub struct NetbenchCoordServerProtocol {
    state: NetbenchCoordServerState,
}

impl NetbenchCoordServerProtocol {
    pub fn new() -> Self {
        NetbenchCoordServerProtocol {
            state: NetbenchCoordServerState::CoordCheckPeer,
        }
    }
}

#[async_trait]
impl Protocol for NetbenchCoordServerProtocol {
    type State = NetbenchCoordServerState;

    async fn connect(&self, addr: &SocketAddr) -> RussulaResult<TcpStream> {
        println!("--- Coordinator: attempt to connect to worker on: {}", addr);

        let connect = TcpStream::connect(addr)
            .await
            .map_err(|err| RussulaError::Connect {
                dbg: err.to_string(),
            })?;

        Ok(connect)
    }

    async fn start(&mut self, stream: &TcpStream) -> RussulaResult<()> {
        self.send_msg(stream, self.state()).await?;
        Ok(())
    }

    async fn recv_msg(&self, stream: &TcpStream) -> RussulaResult<Bytes> {
        stream.readable().await.unwrap();

        let mut buf = Vec::with_capacity(100);
        match stream.try_read_buf(&mut buf) {
            Ok(n) => {
                let msg = NetbenchWorkerServerState::from_bytes(&buf)?;
                println!("read {} bytes: {:?}", n, &msg);
                Ok(Bytes::from_iter(buf))
            }
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

#[derive(Copy, Clone, Debug)]
pub enum NetbenchCoordServerState {
    CoordCheckPeer,
    CoordReady,
    CoordWaitPeerDone,
    CoordDone,
}

impl StateApi for NetbenchCoordServerState {
    fn eq(&self, other: Self) -> bool {
        match self {
            NetbenchCoordServerState::CoordCheckPeer => {
                matches!(other, NetbenchCoordServerState::CoordCheckPeer)
            }
            NetbenchCoordServerState::CoordReady => {
                matches!(other, NetbenchCoordServerState::CoordReady)
            }
            NetbenchCoordServerState::CoordWaitPeerDone => {
                matches!(other, NetbenchCoordServerState::CoordWaitPeerDone)
            }
            NetbenchCoordServerState::CoordDone => {
                matches!(other, NetbenchCoordServerState::CoordDone)
            }
        }
    }

    fn expect_peer_msg(&self) -> Option<NextTransitionMsg> {
        match self {
            NetbenchCoordServerState::CoordCheckPeer => Some(NextTransitionMsg::PeerDriven(
                NetbenchWorkerServerState::ServerReady.as_bytes(),
            )),
            NetbenchCoordServerState::CoordReady => None,
            NetbenchCoordServerState::CoordWaitPeerDone => Some(NextTransitionMsg::PeerDriven(
                NetbenchWorkerServerState::ServerDone.as_bytes(),
            )),
            NetbenchCoordServerState::CoordDone => None,
        }
    }

    fn next(&mut self) {
        match self {
            NetbenchCoordServerState::CoordCheckPeer => NetbenchCoordServerState::CoordReady,
            NetbenchCoordServerState::CoordReady => NetbenchCoordServerState::CoordWaitPeerDone,
            NetbenchCoordServerState::CoordWaitPeerDone => NetbenchCoordServerState::CoordDone,
            NetbenchCoordServerState::CoordDone => NetbenchCoordServerState::CoordDone,
        };
    }

    fn process_msg(&mut self, msg: Bytes) {
        if let Some(NextTransitionMsg::PeerDriven(peer_msg)) = self.expect_peer_msg() {
            println!(
                "coord {:?} {:?}",
                std::str::from_utf8(peer_msg),
                std::str::from_utf8(&msg)
            );
            if peer_msg == msg {
                self.next();
            }
        }
    }
}

impl NetbenchCoordServerState {
    pub fn as_bytes(&self) -> &'static [u8] {
        match self {
            NetbenchCoordServerState::CoordCheckPeer => b"coord_check_peer",
            NetbenchCoordServerState::CoordReady => b"coord_ready",
            NetbenchCoordServerState::CoordWaitPeerDone => b"coord_wait_peer_done",
            NetbenchCoordServerState::CoordDone => b"coord_done",
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> RussulaResult<Self> {
        let state = match bytes {
            b"coord_ready" => NetbenchCoordServerState::CoordReady,
            b"coord_wait_peer_done" => NetbenchCoordServerState::CoordWaitPeerDone,
            b"coord_done" => NetbenchCoordServerState::CoordDone,
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
