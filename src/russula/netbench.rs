// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::NextTransitionMsg;
use crate::russula::StateApi;
use async_trait::async_trait;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};

use crate::russula::error::{RussulaError, RussulaResult};
use crate::russula::protocol::Protocol;

#[derive(Clone, Copy)]
pub struct NetbenchWorkerServerProtocol {
    state: NetbenchWorkerServerState,
    peer_state: NetbenchWorkerServerState,
}

impl NetbenchWorkerServerProtocol {
    pub fn new() -> Self {
        NetbenchWorkerServerProtocol {
            state: NetbenchWorkerServerState::Ready,
            peer_state: NetbenchWorkerServerState::Ready,
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
pub enum NetbenchWorkerServerState {
    Ready,
    WaitPeerDone,
    Done,
}

impl StateApi for NetbenchWorkerServerState {
    fn eq(&self, other: Self) -> bool {
        match self {
            NetbenchWorkerServerState::Ready => matches!(other, NetbenchWorkerServerState::Ready),
            NetbenchWorkerServerState::WaitPeerDone => {
                matches!(other, NetbenchWorkerServerState::WaitPeerDone)
            }
            NetbenchWorkerServerState::Done => matches!(other, NetbenchWorkerServerState::Done),
        }
    }

    fn next_transition_msg(&self) -> Option<NextTransitionMsg> {
        match self {
            NetbenchWorkerServerState::Ready => {
                Some(NextTransitionMsg::PeerDriven("ready_next".to_string()))
            }
            NetbenchWorkerServerState::WaitPeerDone => Some(NextTransitionMsg::PeerDriven(
                "wait_peer_done_next".to_string(),
            )),
            NetbenchWorkerServerState::Done => None,
        }
    }

    fn next(&mut self) {
        let a = match self {
            NetbenchWorkerServerState::Ready => NetbenchWorkerServerState::WaitPeerDone,
            NetbenchWorkerServerState::WaitPeerDone => NetbenchWorkerServerState::Done,
            NetbenchWorkerServerState::Done => NetbenchWorkerServerState::Done,
        };
        *self = a;
    }

    fn process_msg(&mut self, msg: String) {
        if let Some(NextTransitionMsg::PeerDriven(peer_msg)) = self.next_transition_msg() {
            if peer_msg == msg {
                self.next();
            }
        }
    }
}

impl NetbenchWorkerServerState {
    pub fn as_bytes(&self) -> &'static [u8] {
        match self {
            NetbenchWorkerServerState::Ready => b"ready",
            NetbenchWorkerServerState::WaitPeerDone => b"wait_peer_done",
            NetbenchWorkerServerState::Done => b"done",
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> RussulaResult<Self> {
        let state = match bytes {
            b"ready" => NetbenchWorkerServerState::Ready,
            b"wait_peer_done" => NetbenchWorkerServerState::WaitPeerDone,
            b"done" => NetbenchWorkerServerState::Done,
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
    peer_state: NetbenchCoordServerState,
}

impl NetbenchCoordServerProtocol {
    pub fn new() -> Self {
        NetbenchCoordServerProtocol {
            state: NetbenchCoordServerState::Ready,
            peer_state: NetbenchCoordServerState::Ready,
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
pub enum NetbenchCoordServerState {
    Ready,
    WaitPeerDone,
    Done,
}

impl StateApi for NetbenchCoordServerState {
    fn eq(&self, other: Self) -> bool {
        match self {
            NetbenchCoordServerState::Ready => matches!(other, NetbenchCoordServerState::Ready),
            NetbenchCoordServerState::WaitPeerDone => {
                matches!(other, NetbenchCoordServerState::WaitPeerDone)
            }
            NetbenchCoordServerState::Done => matches!(other, NetbenchCoordServerState::Done),
        }
    }

    fn next_transition_msg(&self) -> Option<NextTransitionMsg> {
        match self {
            NetbenchCoordServerState::Ready => None,
            NetbenchCoordServerState::WaitPeerDone => Some(NextTransitionMsg::PeerDriven(
                "wait_peer_done_next".to_string(),
            )),
            NetbenchCoordServerState::Done => None,
        }
    }

    fn next(&mut self) {
        match self {
            NetbenchCoordServerState::Ready => NetbenchCoordServerState::WaitPeerDone,
            NetbenchCoordServerState::WaitPeerDone => NetbenchCoordServerState::Done,
            NetbenchCoordServerState::Done => NetbenchCoordServerState::Done,
        };
    }

    fn process_msg(&mut self, msg: String) {
        if let Some(NextTransitionMsg::PeerDriven(peer_msg)) = self.next_transition_msg() {
            if peer_msg == msg {
                self.next();
            }
        }
    }
}

impl NetbenchCoordServerState {
    pub fn as_bytes(&self) -> &'static [u8] {
        match self {
            NetbenchCoordServerState::Ready => b"ready",
            NetbenchCoordServerState::WaitPeerDone => b"wait_peer_done",
            NetbenchCoordServerState::Done => b"done",
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> RussulaResult<Self> {
        let state = match bytes {
            b"ready" => NetbenchCoordServerState::Ready,
            b"wait_peer_done" => NetbenchCoordServerState::WaitPeerDone,
            b"done" => NetbenchCoordServerState::Done,
            bad_msg => {
                return Err(RussulaError::BadMsg {
                    dbg: format!("unrecognized msg {:?}", std::str::from_utf8(bad_msg)),
                })
            }
        };

        Ok(state)
    }
}

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

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn netbench_state() {}
}
