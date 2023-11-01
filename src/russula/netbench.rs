// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::NextTransitionMsg;
use crate::russula::StateApi;
use async_trait::async_trait;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};

use crate::russula::error::{RussulaError, RussulaResult};
use crate::russula::protocol::Protocol;

// enum NetbenchServerCoordState {
struct CoordCheckPeer;
struct CoordReady;
struct CoordRunPeer;
struct CoordKillPeer;
struct CoordDone;

// enum NetbenchServerWorkerState {
struct ServerWaitPeerReady;
struct ServerReady;
struct ServerRun;
struct ServerDone;

#[allow(non_camel_case_types)]
enum NetbenchServerStateMachine {
    AA_1((CoordCheckPeer, ServerWaitPeerReady)),
    AB_2((CoordCheckPeer, ServerReady)),
    BB_3((CoordReady, ServerReady)),
    CB_4((CoordRunPeer, ServerReady)),
    CC_5((CoordRunPeer, ServerRun)),
    DC_6((CoordKillPeer, ServerRun)),
    DD_7((CoordKillPeer, ServerDone)),
    ED_8((CoordDone, ServerDone)),
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

        self.state.process_msg("ready_next".to_string());

        Ok(())
    }

    async fn recv_msg(&self, stream: &TcpStream) -> RussulaResult<Self::State> {
        stream.readable().await.unwrap();

        let mut buf = Vec::with_capacity(100);
        match stream.try_read_buf(&mut buf) {
            Ok(n) => {
                let msg = NetbenchWorkerServerState::from_bytes(&buf)?;
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

    async fn send_msg(&self, stream: &TcpStream, msg: Self::State) -> RussulaResult<()> {
        stream.writable().await.unwrap();

        stream.try_write(msg.as_bytes()).unwrap();

        Ok(())
    }

    fn state(&self) -> Self::State {
        self.state
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
pub enum NetbenchWorkerServerState {
    ServerWaitCoordInit,
    ServerReady,
    ServerRun,
    ServerDone,
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

    fn process_msg(&mut self, msg: String) {
        if let Some(NextTransitionMsg::PeerDriven(peer_msg)) = self.expect_peer_msg() {
            if peer_msg == msg.as_bytes() {
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

    async fn recv_msg(&self, stream: &TcpStream) -> RussulaResult<Self::State> {
        stream.readable().await.unwrap();

        let mut buf = Vec::with_capacity(100);
        match stream.try_read_buf(&mut buf) {
            Ok(n) => {
                let msg = NetbenchCoordServerState::from_bytes(&buf)?;
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

    fn process_msg(&mut self, msg: String) {
        if let Some(NextTransitionMsg::PeerDriven(peer_msg)) = self.expect_peer_msg() {
            if peer_msg == msg.as_bytes() {
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
