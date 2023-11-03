use async_trait::async_trait;
use bytes::Bytes;
use core::fmt::Debug;
use std::net::SocketAddr;
use tokio::net::TcpStream;

use super::RussulaResult;

pub(crate) struct RussulaPeer<P: Protocol> {
    pub addr: SocketAddr,
    pub stream: TcpStream,
    pub protocol: P,
}

#[async_trait]
pub trait Protocol: Clone + Sync {
    type State: StateApi + Copy + Debug;

    // TODO use version and app to negotiate version
    // fn version(&self) {1, 2}
    // fn app_name(&self) { "netbench" }

    async fn connect(&self, addr: &SocketAddr) -> RussulaResult<TcpStream>;
    async fn run_till_ready(&mut self, stream: &TcpStream) -> RussulaResult<()>;
    async fn run_till_state(&mut self, stream: &TcpStream, state: Self::State)
        -> RussulaResult<()>;
    fn state(&self) -> Self::State;
}

pub type SockProtocol<P> = (SocketAddr, P);

#[derive(Debug)]
pub enum TransitionStep {
    UserDriven,
    PeerDriven(&'static [u8]),
    Finished,
}

#[async_trait]
pub trait StateApi: Sized {
    async fn run(&mut self, stream: &TcpStream);
    fn eq(&self, other: Self) -> bool;
    fn transition_step(&self) -> TransitionStep;
    fn next(&mut self);
    fn process_msg(&mut self, msg: Bytes);
    fn as_bytes(&self) -> &'static [u8];
    fn from_bytes(bytes: &[u8]) -> RussulaResult<Self>;
}
