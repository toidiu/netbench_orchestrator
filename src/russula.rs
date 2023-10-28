use std::{
    collections::BTreeMap,
    collections::BTreeSet,
    net::{SocketAddr, TcpListener, TcpStream},
};

pub struct Russula<P: Protocol> {
    role: Role<P>,
}

impl<P: Protocol> Russula<P> {
    pub fn new_coordinator(addr: BTreeSet<SocketAddr>, protocol: P) -> Self {
        let mut map = BTreeMap::new();
        addr.into_iter().for_each(|addr| {
            map.insert(addr, protocol.clone());
        });
        let role = Role::Coordinator(map);
        Self { role }
    }

    pub fn new_worker(protocol: P) -> Self {
        Self {
            role: Role::Worker(protocol),
        }
    }

    pub async fn connect(&self) {
        match &self.role {
            Role::Coordinator(protocol_map) => {
                for (addr, protocol) in protocol_map.iter() {
                    protocol.connect_to_worker(*addr)
                }
            }
            Role::Worker(protocol) => protocol.wait_for_coordinator(),
        }
    }

    pub async fn start(&self) {
        match &self.role {
            Role::Coordinator(_role) => todo!(),
            Role::Worker(_role) => todo!(),
        }
    }

    pub async fn kill(&self) {
        match &self.role {
            Role::Coordinator(_) => todo!(),
            Role::Worker(role) => role.kill(),
        }
    }

    pub async fn wait_peer_state(&self, _state: P::Message) {}
}

pub trait Protocol: Clone {
    type Message;

    // TODO replace u8 with uuid
    fn id(&self) -> u8 {
        0
    }
    fn version(&self) {}
    fn app(&self) {}

    fn connect_to_worker(&self, _addr: SocketAddr);
    fn wait_for_coordinator(&self);

    fn start(&self) {}
    fn kill(&self) {}

    fn recv(&self) {}
    fn send(&self) {}
    fn peer_state(&self) -> Self::Message;
}

enum Role<P: Protocol> {
    Coordinator(BTreeMap<SocketAddr, P>),
    Worker(P),
}

#[derive(Clone, Copy)]
pub struct NetbenchOrchestrator {
    peer_state: NetbenchState,
}

impl NetbenchOrchestrator {
    pub fn new() -> Self {
        NetbenchOrchestrator {
            peer_state: NetbenchState::Ready,
        }
    }
}

impl Protocol for NetbenchOrchestrator {
    type Message = NetbenchState;

    fn wait_for_coordinator(&self) {
        let listener = TcpListener::bind("127.0.0.1:8900").unwrap();
        let conn = listener.accept().unwrap();
    }

    fn connect_to_worker(&self, addr: SocketAddr) {
        // FIXME fix this
        let conn = TcpStream::connect(addr).unwrap();
    }

    fn peer_state(&self) -> Self::Message {
        self.peer_state
    }
}

#[derive(Copy, Clone)]
pub enum NetbenchState {
    Ready,
    Run,
    Done,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    //     #[derive(Copy, Clone)]
    //     struct TestProtocol;

    //     enum TestProtocolMessage {
    //         Ready,
    //         Done,
    //     }

    //     impl Protocol for TestProtocol {
    //         type Message = TestProtocolMessage;

    //         fn peer_state(&self) -> Self::Message {
    //             todo!()
    //         }
    //     }

    #[tokio::test]
    async fn test() {
        let test_protocol = NetbenchOrchestrator::new();
        let addr = BTreeSet::from_iter([SocketAddr::from_str("127.0.0.1:8900").unwrap()]);
        let _coord = Russula::new_coordinator(addr, test_protocol)
            .connect()
            .await;

        let _worker = Russula::new_worker(test_protocol).connect().await;

        assert!(1 == 1)
    }
}
