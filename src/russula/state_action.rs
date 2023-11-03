// #![allow(unused_imports)]
// #![allow(unused)]
// use super::protocol::StateApi;
// use crate::russula::network_utils;
// use crate::russula::Protocol;
// use crate::russula::RussulaResult;
// use crate::russula::TransitionStep;
// use async_trait::async_trait;
// use bytes::Bytes;
// use core::fmt::Debug;
// use std::net::SocketAddr;
// use tokio::net::TcpStream;

// #[async_trait]
// pub trait StateAction: Debug + Sync + Send {
//     async fn run_action(&self, stream: &TcpStream, state: Bytes);
// }
// #[derive(Clone, Debug)]
// pub struct NotifyPeer;
// #[async_trait]
// impl StateAction for NotifyPeer {
//     async fn run_action(&self, stream: &TcpStream, state: Bytes) {
//         network_utils::send_msg(stream, state).await.unwrap();
//     }
// }
// #[derive(Clone, Debug)]
// pub struct WaitPeerState;
// #[async_trait]
// impl StateAction for WaitPeerState {
//     async fn run_action(&self, stream: &TcpStream, state: Bytes) {
//         network_utils::recv_msg(stream).await.unwrap()
//     }
// }

// #[derive(Debug)]
// pub enum MyProtocolState {
//     CheckPeer(Vec<Box<dyn StateAction>>),
// }

// unsafe impl Send for MyProtocolState {}

// impl Clone for MyProtocolState {
//     fn clone(&self) -> Self {
//         match self {
//             Self::CheckPeer(_) => {
//                 let a: Vec<Box<dyn StateAction>> =
//                     vec![Box::new(NotifyPeer {}), Box::new(NotifyPeer {})];
//                 Self::CheckPeer(a)
//             }
//         }
//     }
// }

// #[async_trait]
// impl StateApi for MyProtocolState {
//     async fn run(&mut self, stream: &TcpStream) {}
//     fn eq(&self, other: &Self) -> bool {
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

// impl MyProtocolState {
//     fn run(&self) {
//         let state =
//             MyProtocolState::CheckPeer(vec![Box::new(NotifyPeer {}), Box::new(WaitPeerState {})]);
//         match state {
//             MyProtocolState::CheckPeer(v) => v.iter().for_each(|s| {
//                 // let stream = todo!();
//                 // s.run_action(stream);
//             }),
//         }
//     }
// }

// #[derive(Clone, Debug)]
// struct MyProtocol {
//     state: MyProtocolState,
// }

// unsafe impl Sync for MyProtocol {}
// unsafe impl Send for MyProtocol {}

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
//     fn state(&self) -> &Self::State {
//         todo!()
//     }
// }
