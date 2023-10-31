use async_trait::async_trait;
use std::net::SocketAddr;
use tokio::net::TcpStream;

use super::RussulaResult;

#[async_trait]
pub trait Protocol: Clone {
    type State: StateApi + Copy;

    // TODO replace u8 with uuid
    fn id(&self) -> u8 {
        0
    }
    // fn version(&self) {}
    // fn app(&self) {}

    async fn connect_to_worker(&self, addr: SocketAddr) -> RussulaResult<TcpStream>;
    async fn wait_for_coordinator(&self, addr: &SocketAddr) -> RussulaResult<TcpStream>;
    async fn recv_msg(&self, stream: TcpStream) -> RussulaResult<Self::State>;
    async fn send_msg(&self, stream: TcpStream, msg: Self::State) -> RussulaResult<()>;

    fn start(&self) {}
    fn kill(&self) {}

    fn state(&self) -> Self::State;
    fn peer_state(&self) -> Self::State;
}

type SockProtocol<P> = (SocketAddr, P);

pub enum Role<P: Protocol> {
    Coordinator(Vec<SockProtocol<P>>),
    Worker(SockProtocol<P>),
}

pub enum NextTransitionMsg {
    SelfDriven,
    PeerDriven(String),
}

pub trait StateApi {
    fn curr(&self) -> &Self;
    fn next_transition_msg(&self) -> Option<NextTransitionMsg>;
    fn next(&mut self) -> Self;
    fn process_msg(&mut self, msg: String) -> &Self;
}