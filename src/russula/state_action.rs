// use super::protocol::StateApi;
// use crate::russula::Protocol;
// use crate::russula::RussulaResult;
// use crate::russula::TransitionStep;
// use async_trait::async_trait;
// use bytes::Bytes;
// use std::net::SocketAddr;
// use std::net::TcpStream;

// pub trait StateAction {
//     fn run(&self) {}
// }
// #[derive(Clone, Debug)]
// pub struct NotifyPeer {}
// impl StateAction for NotifyPeer {}
// pub struct WaitPeerState {}
// impl StateAction for WaitPeerState {}

// #[derive(Clone)]
// pub enum MyProtocolState {
//     CheckPeer(Vec<Box<dyn StateAction>>),
// }

// #[async_trait]
// impl StateApi for MyProtocolState {
//     async fn run(&mut self, stream: &TcpStream) {
//         todo!()
//     }
//     fn eq(&self, other: Self) -> bool {
//         todo!()
//     }
//     fn transition_step(&self) -> TransitionStep {
//         todo!()
//     }
//     fn next(&mut self) {
//         todo!()
//     }
//     fn process_msg(&mut self, msg: Bytes) {
//         todo!()
//     }
//     fn as_bytes(&self) -> &'static [u8] {
//         todo!()
//     }
//     fn from_bytes(bytes: &[u8]) -> RussulaResult<Self> {
//         todo!()
//     }
// }

// // impl CoordServerState {
// //     fn run(&self) {
// //         let a =
// //             CoordServerState::CheckPeer(vec![Box::new(NotifyPeer {}), Box::new(WaitPeerState {})]);
// //         match a {
// //             CoordServerState::CheckPeer(v) => v.iter().for_each(|s| {
// //                 s.run();
// //             }),
// //         }
// //     }
// // }

// #[derive(Clone, Debug)]
// struct MyProtocol {
//     state: MyProtocolState,
// }

// unsafe impl Sync for MyProtocol {}

// #[async_trait]
// impl Protocol for MyProtocol {
//     type State = MyProtocolState;

//     async fn connect(&self, addr: &SocketAddr) -> RussulaResult<TcpStream> {
//         todo!()
//     }
//     async fn run_till_ready(&mut self, stream: &TcpStream) -> RussulaResult<()> {
//         todo!()
//     }
//     async fn run_till_done(&mut self, stream: &TcpStream) -> RussulaResult<()> {
//         todo!()
//     }
//     async fn run_till_state(
//         &mut self,
//         stream: &TcpStream,
//         state: Self::State,
//     ) -> RussulaResult<()> {
//         todo!()
//     }
//     fn state(&self) -> Self::State {
//         todo!()
//     }
// }
