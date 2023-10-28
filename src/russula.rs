use std::{collections::BTreeMap, collections::BTreeSet, net::SocketAddr};

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

    pub async fn connect(&self) -> Self {
        match self.role {
            Role::Coordinator(_) => todo!(),
            Role::Worker(_) => todo!(),
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

#[derive(Clone)]
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
