pub trait StateAction {
    fn run(&self) {}
}
#[derive(Clone, Debug)]
pub struct NotifyPeer {}
impl StateAction for NotifyPeer {}
pub struct WaitPeerState {}
impl StateAction for WaitPeerState {}

pub enum CoordServerState {
    CheckPeer(Vec<Box<dyn StateAction>>),
}
// impl CoordServerState {
//     fn run(&self) {
//         let a =
//             CoordServerState::CheckPeer(vec![Box::new(NotifyPeer {}), Box::new(WaitPeerState {})]);
//         match a {
//             CoordServerState::CheckPeer(v) => v.iter().for_each(|s| {
//                 s.run();
//             }),
//         }
//     }
// }
