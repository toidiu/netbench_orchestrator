use async_trait::async_trait;
use bytes::Bytes;
use core::fmt::Debug;
use std::net::SocketAddr;
use tokio::net::TcpStream;

use super::RussulaResult;

#[async_trait]
pub trait Protocol: Clone + Sync {
    type State: StateApi + Copy + Debug;

    // TODO replace u8 with uuid
    fn id(&self) -> u8 {
        0
    }
    // fn version(&self) {1, 2}
    // fn app(&self) { "netbench" }

    async fn connect(&self, addr: &SocketAddr) -> RussulaResult<TcpStream>;
    async fn run_till_ready(&mut self, stream: &TcpStream) -> RussulaResult<()>;
    async fn run_till_state(&mut self, stream: &TcpStream, state: Self::State) -> RussulaResult<()>;

    async fn recv_msg(&self, stream: &TcpStream) -> RussulaResult<Bytes>;
    async fn send_msg(&self, stream: &TcpStream, msg: Self::State) -> RussulaResult<()>;

    fn state(&self) -> Self::State;
}

pub type SockProtocol<P> = (SocketAddr, P);

#[derive(Debug)]
pub enum NextTransitionMsg {
    SelfDriven,
    PeerDriven(&'static [u8]),
}

#[async_trait]
pub trait StateApi {
    async fn run(&mut self, stream: &TcpStream) {}
    fn eq(&self, other: Self) -> bool;
    fn expect_peer_msg(&self) -> Option<NextTransitionMsg>;
    fn next(&mut self);
    fn process_msg(&mut self, msg: Bytes);
}
