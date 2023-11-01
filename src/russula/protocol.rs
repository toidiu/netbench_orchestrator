use async_trait::async_trait;
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
    async fn start(&mut self, stream: &TcpStream) -> RussulaResult<()>;

    async fn recv_msg(&self, stream: &TcpStream) -> RussulaResult<Self::State>;
    async fn send_msg(&self, stream: &TcpStream, msg: Self::State) -> RussulaResult<()>;

    fn state(&self) -> Self::State;
    fn peer_state(&self) -> Self::State;
}

pub type SockProtocol<P> = (SocketAddr, P);

pub enum NextTransitionMsg {
    SelfDriven,
    PeerDriven(String),
}

pub trait StateApi {
    fn eq(&self, other: Self) -> bool;
    fn curr(&self) -> &Self {
        self
    }
    fn next_transition_msg(&self) -> Option<NextTransitionMsg>;
    fn next(&mut self);
    fn process_msg(&mut self, msg: String);
}
